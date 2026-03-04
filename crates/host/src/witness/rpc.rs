//! RPC-based witness provider using kona-host for preimage collection.
//!
//! Connects to L1 execution, L2 execution, L1 beacon, and (optionally) OP Node
//! endpoints to collect all preimages needed for the guest derivation pipeline.
//!
//! Follows the kailua/op-succinct pattern:
//! 1. Fetch rollup config from OP Node RPC (`optimism_rollupConfig`)
//! 2. Write to temp file so kona's `SingleChainLocalInputs` can serve it
//! 3. Fetch output roots from OP Node (`optimism_outputAtBlock`)
//! 4. Run kona client + preimage server, abort server after client completes

use std::path::PathBuf;
use std::sync::Arc;

use alloy_primitives::{keccak256, B256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types_eth::BlockNumberOrTag;
use async_trait::async_trait;
use kona_host::single::{SingleChainHintHandler, SingleChainHost, SingleChainLocalInputs};
use kona_host::{
    OnlineHostBackend, PreimageServer, SharedKeyValueStore, SplitKeyValueStore,
};
use kona_preimage::{BidirectionalChannel, HintReader, HintWriter, OracleReader, OracleServer};
use open_zk_core::traits::{RawWitness, WitnessProvider};
use open_zk_core::types::BootInfo;
use open_zk_core::types::ProofRequest;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::kv_store::{serialize_preimages, ArcMemoryKvStore};

/// RPC-based witness provider that fetches L1/L2 data via kona-host.
///
/// Runs the kona fault proof program natively to discover all required
/// preimages, then serializes them into a [`RawWitness`] for offline proving.
pub struct RpcWitnessProvider {
    l1_rpc_url: String,
    l2_rpc_url: String,
    l1_beacon_url: String,
    /// OP Node RPC URL for `optimism_rollupConfig` and `optimism_outputAtBlock`.
    /// If None, output roots fall back to block header derivation.
    op_node_url: Option<String>,
    /// Optional rollup config path. If None, fetched from OP Node.
    rollup_config_path: Option<PathBuf>,
    /// Optional L2 chain ID. If None, determined from rollup config.
    l2_chain_id: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
pub enum RpcWitnessError {
    #[error("RPC connection failed: {0}")]
    Connection(String),
    #[error("data fetching failed: {0}")]
    Fetch(String),
    #[error("witness serialization failed: {0}")]
    Serialization(String),
    #[error("kona host error: {0}")]
    KonaHost(String),
    #[error("kona client error: {0}")]
    KonaClient(String),
}

impl RpcWitnessProvider {
    /// Create a new RPC witness provider.
    pub fn new(l1_rpc_url: String, l2_rpc_url: String, l1_beacon_url: String) -> Self {
        Self {
            l1_rpc_url,
            l2_rpc_url,
            l1_beacon_url,
            op_node_url: None,
            rollup_config_path: None,
            l2_chain_id: None,
        }
    }

    /// Set the OP Node RPC URL (for `optimism_rollupConfig` and `optimism_outputAtBlock`).
    pub fn with_op_node_url(mut self, url: String) -> Self {
        self.op_node_url = Some(url);
        self
    }

    /// Set the rollup config path.
    pub fn with_rollup_config(mut self, path: PathBuf) -> Self {
        self.rollup_config_path = Some(path);
        self
    }

    /// Set the L2 chain ID.
    pub fn with_chain_id(mut self, chain_id: u64) -> Self {
        self.l2_chain_id = Some(chain_id);
        self
    }

    /// Ensure a valid rollup config file exists and return its absolute path.
    ///
    /// Following the kailua pattern:
    /// 1. If `rollup_config_path` is set and readable, canonicalize and return it
    /// 2. Otherwise, fetch from OP Node via `optimism_rollupConfig` RPC
    /// 3. Merge fork activation times from `debug_chainConfig` (L2 node)
    /// 4. Write to temp file and return the path
    async fn ensure_rollup_config(&self) -> Result<PathBuf, RpcWitnessError> {
        // Try existing path first
        if let Some(path) = &self.rollup_config_path {
            match std::fs::canonicalize(path) {
                Ok(abs_path) => {
                    // Verify it's valid JSON by attempting to read it
                    match std::fs::read_to_string(&abs_path) {
                        Ok(contents) => {
                            if serde_json::from_str::<serde_json::Value>(&contents).is_ok() {
                                debug!(path = %abs_path.display(), "using existing rollup config");
                                return Ok(abs_path);
                            }
                            warn!(path = %abs_path.display(), "rollup config is not valid JSON");
                        }
                        Err(e) => {
                            warn!(path = %abs_path.display(), error = %e, "cannot read rollup config");
                        }
                    }
                }
                Err(e) => {
                    warn!(path = %path.display(), error = %e, "rollup config path not found");
                }
            }
        }

        // Fetch from OP Node RPC
        let op_node_url = self.op_node_url.as_ref().ok_or_else(|| {
            RpcWitnessError::Fetch(
                "no rollup config path or OP Node URL provided; \
                 set one via with_rollup_config() or with_op_node_url()"
                    .to_string(),
            )
        })?;

        info!(op_node = %op_node_url, "fetching rollup config from OP Node");

        let op_url: url::Url = op_node_url
            .parse()
            .map_err(|e| RpcWitnessError::Connection(format!("invalid OP Node URL: {e}")))?;
        let op_provider = ProviderBuilder::new().connect_http(op_url);

        // Fetch rollup config via optimism_rollupConfig
        let mut rollup_config: serde_json::Value = op_provider
            .raw_request::<_, serde_json::Value>("optimism_rollupConfig".into(), ())
            .await
            .map_err(|e| {
                RpcWitnessError::Fetch(format!("optimism_rollupConfig RPC failed: {e}"))
            })?;

        // Merge fork activation times from L2 node's debug_chainConfig (kailua pattern).
        // debug_chainConfig returns camelCase (e.g., "canyonTime") but kona's RollupConfig
        // expects snake_case (e.g., "canyon_time"). Only merge if the snake_case key is missing.
        let l2_url: url::Url = self
            .l2_rpc_url
            .parse()
            .map_err(|e| RpcWitnessError::Connection(format!("invalid L2 URL: {e}")))?;
        let l2_provider = ProviderBuilder::new().connect_http(l2_url);

        if let Ok(chain_config) = l2_provider
            .raw_request::<_, serde_json::Value>("debug_chainConfig".into(), ())
            .await
        {
            // Map: (camelCase from debug_chainConfig) → (snake_case for kona RollupConfig)
            let fork_mappings = [
                ("regolithTime", "regolith_time"),
                ("canyonTime", "canyon_time"),
                ("deltaTime", "delta_time"),
                ("ecotoneTime", "ecotone_time"),
                ("fjordTime", "fjord_time"),
                ("graniteTime", "granite_time"),
                ("holoceneTime", "holocene_time"),
                ("isthmusTime", "isthmus_time"),
            ];
            for (camel, snake) in &fork_mappings {
                // Only add if the snake_case key is missing from rollup config
                if rollup_config.get(*snake).is_none() {
                    if let Some(value) = chain_config.get(*camel) {
                        if !value.is_null() {
                            rollup_config[*snake] = value.clone();
                        }
                    }
                }
            }
            debug!("merged fork activation times from debug_chainConfig");
        } else {
            debug!("debug_chainConfig not available, using rollup config as-is");
        }

        // Write to temp file
        let tmp_path = std::env::temp_dir().join(format!(
            "open-zk-rollup-config-{}.json",
            std::process::id()
        ));
        let config_str = serde_json::to_string_pretty(&rollup_config).map_err(|e| {
            RpcWitnessError::Serialization(format!("rollup config serialization: {e}"))
        })?;
        std::fs::write(&tmp_path, &config_str).map_err(|e| {
            RpcWitnessError::Fetch(format!("failed to write rollup config to {}: {e}", tmp_path.display()))
        })?;

        info!(path = %tmp_path.display(), "wrote rollup config to temp file");
        Ok(tmp_path)
    }

    /// Ensure a valid L1 chain config file exists and return its absolute path.
    ///
    /// kona's `SingleChainLocalInputs` serves L1 config via `L1_CONFIG_KEY` preimage.
    /// For known chains (mainnet, sepolia, holesky) the kona client uses its built-in
    /// registry, but for devnets we must provide the config file.
    ///
    /// Tries `debug_chainConfig` on the L1 node first. If unavailable, generates a
    /// default config with all post-merge hardforks activated from genesis.
    async fn ensure_l1_config(
        &self,
        l1_chain_id: u64,
    ) -> Result<PathBuf, RpcWitnessError> {
        let l1_url: url::Url = self
            .l1_rpc_url
            .parse()
            .map_err(|e| RpcWitnessError::Connection(format!("invalid L1 URL: {e}")))?;
        let l1_provider = ProviderBuilder::new().connect_http(l1_url);

        // Try debug_chainConfig from L1 node (geth-compatible JSON)
        let l1_config: serde_json::Value =
            if let Ok(config) = l1_provider
                .raw_request::<_, serde_json::Value>("debug_chainConfig".into(), ())
                .await
            {
                debug!("fetched L1 chain config from debug_chainConfig");
                config
            } else {
                // Generate default config for devnet (all forks activated from genesis).
                // Uses camelCase field names matching alloy_genesis::ChainConfig serde format.
                debug!(chain_id = l1_chain_id, "generating default L1 chain config for devnet");
                serde_json::json!({
                    "chainId": l1_chain_id,
                    "homesteadBlock": 0,
                    "eip150Block": 0,
                    "eip155Block": 0,
                    "eip158Block": 0,
                    "byzantiumBlock": 0,
                    "constantinopleBlock": 0,
                    "petersburgBlock": 0,
                    "istanbulBlock": 0,
                    "berlinBlock": 0,
                    "londonBlock": 0,
                    "shanghaiTime": 0,
                    "cancunTime": 0,
                    "terminalTotalDifficulty": "0",
                    "terminalTotalDifficultyPassed": true
                })
            };

        let tmp_path = std::env::temp_dir().join(format!(
            "open-zk-l1-config-{}.json",
            std::process::id()
        ));
        let config_str = serde_json::to_string_pretty(&l1_config).map_err(|e| {
            RpcWitnessError::Serialization(format!("l1 config serialization: {e}"))
        })?;
        std::fs::write(&tmp_path, &config_str).map_err(|e| {
            RpcWitnessError::Fetch(format!(
                "failed to write L1 config to {}: {e}",
                tmp_path.display()
            ))
        })?;

        info!(path = %tmp_path.display(), chain_id = l1_chain_id, "wrote L1 chain config to temp file");
        Ok(tmp_path)
    }

    /// Build a [`SingleChainHost`] configuration from a proof request.
    ///
    /// Maps open-zk's `ProofRequest` fields to kona-host's `SingleChainHost`
    /// parameters. Some fields (agreed_l2_head_hash, claimed_l2_output_root)
    /// must be fetched from L2 RPC before calling this.
    fn build_host_config(
        &self,
        request: &ProofRequest,
        agreed_l2_head_hash: B256,
        claimed_l2_output_root: B256,
        rollup_config_path: PathBuf,
        l1_config_path: PathBuf,
    ) -> SingleChainHost {
        SingleChainHost {
            l1_head: request.l1_head,
            agreed_l2_head_hash,
            agreed_l2_output_root: request.l2_start_output_root,
            claimed_l2_output_root,
            claimed_l2_block_number: request.l2_end_block,
            l1_node_address: Some(self.l1_rpc_url.clone()),
            l2_node_address: Some(self.l2_rpc_url.clone()),
            l1_beacon_address: Some(self.l1_beacon_url.clone()),
            native: true,
            server: false,
            l2_chain_id: self.l2_chain_id,
            rollup_config_path: Some(rollup_config_path),
            l1_config_path: Some(l1_config_path),
            data_dir: None, // Use in-memory KV store
            ..Default::default()
        }
    }

    /// Compute a hash of the rollup config for the guest BootInfo.
    fn compute_rollup_config_hash(
        &self,
        rollup_config_path: &PathBuf,
    ) -> Result<B256, RpcWitnessError> {
        let contents = std::fs::read(rollup_config_path).map_err(|e| {
            RpcWitnessError::Fetch(format!(
                "failed to read rollup config at {}: {e}",
                rollup_config_path.display()
            ))
        })?;
        // Normalize JSON: parse and re-serialize to get canonical form
        let value: serde_json::Value = serde_json::from_slice(&contents).map_err(|e| {
            RpcWitnessError::Fetch(format!("invalid rollup config JSON: {e}"))
        })?;
        let canonical = serde_json::to_vec(&value).map_err(|e| {
            RpcWitnessError::Serialization(format!("rollup config serialization: {e}"))
        })?;
        Ok(keccak256(&canonical))
    }

    /// Fetch the L2 output root at a given block number.
    ///
    /// Tries `optimism_outputAtBlock` on OP Node first (correct endpoint).
    /// Falls back to block header derivation from L2 execution node.
    async fn fetch_output_root(
        &self,
        l2_provider: &impl Provider,
        block_number: u64,
    ) -> Result<B256, RpcWitnessError> {
        // Try optimism_outputAtBlock on OP Node (the correct endpoint)
        if let Some(op_url) = &self.op_node_url {
            let url: url::Url = op_url
                .parse()
                .map_err(|e| RpcWitnessError::Connection(format!("invalid OP Node URL: {e}")))?;
            let op_provider = ProviderBuilder::new().connect_http(url);

            let rpc_result: Result<serde_json::Value, _> = op_provider
                .raw_request(
                    "optimism_outputAtBlock".into(),
                    [format!("0x{:x}", block_number)],
                )
                .await;

            if let Ok(resp) = rpc_result {
                if let Some(root_str) = resp.get("outputRoot").and_then(|v| v.as_str()) {
                    if let Ok(root) = root_str.parse::<B256>() {
                        debug!(block = block_number, output_root = %root, "fetched output root from OP Node");
                        return Ok(root);
                    }
                }
            }
            warn!(block = block_number, "optimism_outputAtBlock failed on OP Node, trying L2 node");
        }

        // Try optimism_outputAtBlock on L2 node (some nodes support this)
        let rpc_result: Result<serde_json::Value, _> = l2_provider
            .raw_request(
                "optimism_outputAtBlock".into(),
                [format!("0x{:x}", block_number)],
            )
            .await;

        if let Ok(resp) = rpc_result {
            if let Some(root_str) = resp.get("outputRoot").and_then(|v| v.as_str()) {
                if let Ok(root) = root_str.parse::<B256>() {
                    debug!(block = block_number, output_root = %root, "fetched output root from L2 node");
                    return Ok(root);
                }
            }
        }

        // Fallback: derive from block header
        debug!(
            block = block_number,
            "optimism_outputAtBlock unavailable, deriving from block header"
        );
        let block = l2_provider
            .get_block_by_number(BlockNumberOrTag::Number(block_number))
            .await
            .map_err(|e| RpcWitnessError::Fetch(format!("L2 block {block_number}: {e}")))?
            .ok_or_else(|| {
                RpcWitnessError::Fetch(format!("L2 block {block_number} not found"))
            })?;

        // output_root = keccak256(version_byte[32] ++ state_root ++ withdrawals_root ++ block_hash)
        let mut payload = [0u8; 128];
        // bytes 0..32: version (zero)
        payload[32..64].copy_from_slice(block.header.state_root.as_slice());
        let withdrawals_root = block.header.withdrawals_root.unwrap_or(B256::ZERO);
        payload[64..96].copy_from_slice(withdrawals_root.as_slice());
        payload[96..128].copy_from_slice(block.header.hash.as_slice());
        let root = keccak256(payload);
        debug!(block = block_number, output_root = %root, "derived output root from block header");
        Ok(root)
    }

    /// Run the kona witness collection pipeline.
    ///
    /// Creates an in-memory KV store, starts the preimage server and kona client
    /// concurrently, and collects all preimages accessed during derivation.
    /// Aborts the server task after the client completes (kailua/op-succinct pattern).
    async fn collect_witness(
        &self,
        host: &SingleChainHost,
    ) -> Result<ArcMemoryKvStore, RpcWitnessError> {
        // Verify rollup config is readable before starting the pipeline.
        // SingleChainLocalInputs::get() calls host.read_rollup_config() lazily —
        // if it fails, the client hangs waiting for the preimage.
        host.read_rollup_config().map_err(|e| {
            RpcWitnessError::KonaHost(format!(
                "rollup config not readable (SingleChainLocalInputs will fail): {e}"
            ))
        })?;
        host.read_l1_config().map_err(|e| {
            RpcWitnessError::KonaHost(format!(
                "L1 config not readable (SingleChainLocalInputs will fail): {e}"
            ))
        })?;
        debug!("rollup config and L1 config verified readable");

        // Create our KV store that retains access to collected preimages
        let our_store = ArcMemoryKvStore::new();
        let local_kv = SingleChainLocalInputs::new(host.clone());
        let split_kv = SplitKeyValueStore::new(local_kv, our_store.clone());
        let kv: SharedKeyValueStore = Arc::new(RwLock::new(split_kv));

        // Create RPC providers for data fetching
        let providers = host
            .create_providers()
            .await
            .map_err(|e| RpcWitnessError::Connection(e.to_string()))?;

        // Create bidirectional channels for oracle and hint protocols
        let hint_channel = BidirectionalChannel::new()
            .map_err(|e| RpcWitnessError::Connection(e.to_string()))?;
        let preimage_channel = BidirectionalChannel::new()
            .map_err(|e| RpcWitnessError::Connection(e.to_string()))?;

        // Create the online host backend that fetches preimages from RPC on demand
        let backend =
            OnlineHostBackend::<SingleChainHost, SingleChainHintHandler>::new(
                host.clone(),
                kv,
                providers,
                SingleChainHintHandler,
            );

        // Create the preimage server (handles oracle requests + hint routing)
        let server = PreimageServer::new(
            OracleServer::new(preimage_channel.host),
            HintReader::new(hint_channel.host),
            Arc::new(backend),
        );

        info!(
            l2_end = host.claimed_l2_block_number,
            "starting kona witness collection"
        );

        // Run server and client concurrently.
        // Server is spawned; client is awaited. After client finishes, abort server.
        // This follows the kailua/op-succinct pattern — the server hangs indefinitely
        // if not explicitly terminated after the client completes.
        let server_task = tokio::spawn(async move { server.start().await });
        let client_task = tokio::spawn(kona_client::single::run(
            OracleReader::new(preimage_channel.client),
            HintWriter::new(hint_channel.client),
        ));

        // Wait for the client to finish first
        let client_result = client_task.await;

        // Abort the server — it blocks on channel reads and won't terminate on its own
        server_task.abort();

        // Handle client result
        match client_result {
            Ok(Ok(())) => {
                info!("kona client completed successfully");
            }
            Ok(Err(e)) => {
                return Err(RpcWitnessError::KonaClient(e.to_string()));
            }
            Err(e) => {
                return Err(RpcWitnessError::KonaClient(format!(
                    "client task panicked: {e}"
                )));
            }
        }

        info!(
            preimage_count = our_store.len(),
            "witness collection complete"
        );

        Ok(our_store)
    }
}

#[async_trait]
impl WitnessProvider for RpcWitnessProvider {
    type Error = RpcWitnessError;

    async fn generate_witness(
        &self,
        request: &ProofRequest,
    ) -> Result<RawWitness, Self::Error> {
        // Step 1: Ensure rollup config is available (fetch from OP Node if needed)
        let rollup_config_path = self.ensure_rollup_config().await?;

        // Step 2: Compute rollup config hash from the resolved config
        let rollup_config_hash = self.compute_rollup_config_hash(&rollup_config_path)?;

        // Step 3: Pre-flight — fetch L2 head hash and claimed output root
        let l2_url: url::Url = self
            .l2_rpc_url
            .parse()
            .map_err(|e| RpcWitnessError::Connection(format!("invalid L2 URL: {e}")))?;
        let l2_provider = ProviderBuilder::new().connect_http(l2_url);

        // Fetch block hash at l2_start_block (agreed L2 head)
        let start_block = l2_provider
            .get_block_by_number(BlockNumberOrTag::Number(request.l2_start_block))
            .await
            .map_err(|e| RpcWitnessError::Fetch(format!("L2 start block: {e}")))?
            .ok_or_else(|| {
                RpcWitnessError::Fetch(format!(
                    "L2 block {} not found",
                    request.l2_start_block
                ))
            })?;
        let agreed_l2_head_hash = start_block.header.hash;
        debug!(
            block = request.l2_start_block,
            hash = %agreed_l2_head_hash,
            "fetched L2 start block hash"
        );

        // Fetch output root at l2_end_block (from OP Node or fallback)
        let claimed_l2_output_root = self
            .fetch_output_root(&l2_provider, request.l2_end_block)
            .await?;
        debug!(
            block = request.l2_end_block,
            output_root = %claimed_l2_output_root,
            "determined L2 claimed output root"
        );

        // Step 4: Ensure L1 chain config is available.
        // Parse rollup config to extract L1 chain ID for the L1 config lookup.
        let rollup_json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&rollup_config_path).map_err(|e| {
                RpcWitnessError::Fetch(format!("re-read rollup config: {e}"))
            })?,
        )
        .map_err(|e| RpcWitnessError::Fetch(format!("re-parse rollup config: {e}")))?;
        let l1_chain_id = rollup_json
            .get("l1_chain_id")
            .and_then(|v| v.as_u64())
            .unwrap_or(1); // default to mainnet L1
        let l1_config_path = self.ensure_l1_config(l1_chain_id).await?;

        // Step 5: Build kona host configuration with resolved config paths
        let host = self.build_host_config(
            request,
            agreed_l2_head_hash,
            claimed_l2_output_root,
            rollup_config_path,
            l1_config_path,
        );

        // Step 6: Run the witness collection pipeline
        let store = self.collect_witness(&host).await?;

        // Step 7: Extract and serialize preimages
        let preimages = store.snapshot();
        let oracle_data = serialize_preimages(&preimages);

        // Build BootInfo for the guest program
        let boot_info = BootInfo {
            l1_head: request.l1_head,
            l2_pre_root: request.l2_start_output_root,
            l2_claim: claimed_l2_output_root,
            l2_block_number: request.l2_end_block,
            rollup_config_hash,
        };

        Ok(RawWitness {
            boot_info: boot_info.to_abi_bytes(),
            oracle_data,
            blob_data: vec![], // Blob data is included in oracle preimages
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_witness_provider_construction() {
        let provider = RpcWitnessProvider::new(
            "http://localhost:8545".to_string(),
            "http://localhost:9545".to_string(),
            "http://localhost:5052".to_string(),
        );
        assert_eq!(provider.l1_rpc_url, "http://localhost:8545");
        assert_eq!(provider.l2_rpc_url, "http://localhost:9545");
        assert_eq!(provider.l1_beacon_url, "http://localhost:5052");
        assert!(provider.op_node_url.is_none());
    }

    #[test]
    fn rpc_witness_provider_with_config() {
        let provider = RpcWitnessProvider::new(
            "http://l1:8545".to_string(),
            "http://l2:9545".to_string(),
            "http://beacon:5052".to_string(),
        )
        .with_op_node_url("http://op-node:7545".to_string())
        .with_rollup_config(PathBuf::from("/etc/rollup.json"))
        .with_chain_id(10);

        assert_eq!(
            provider.rollup_config_path,
            Some(PathBuf::from("/etc/rollup.json"))
        );
        assert_eq!(provider.l2_chain_id, Some(10));
        assert_eq!(
            provider.op_node_url,
            Some("http://op-node:7545".to_string())
        );
    }

    #[test]
    fn build_host_config_maps_fields() {
        let provider = RpcWitnessProvider::new(
            "http://l1:8545".to_string(),
            "http://l2:9545".to_string(),
            "http://beacon:5052".to_string(),
        )
        .with_chain_id(420);

        let request = ProofRequest {
            l1_head: B256::repeat_byte(0x11),
            l2_start_block: 100,
            l2_end_block: 200,
            l2_start_output_root: B256::repeat_byte(0x22),
            mode: open_zk_core::types::ProvingMode::Groth16,
        };

        let host = provider.build_host_config(
            &request,
            B256::repeat_byte(0x33),
            B256::repeat_byte(0x44),
            PathBuf::from("/tmp/rollup.json"),
            PathBuf::from("/tmp/l1-config.json"),
        );

        assert_eq!(host.l1_head, B256::repeat_byte(0x11));
        assert_eq!(host.agreed_l2_output_root, B256::repeat_byte(0x22));
        assert_eq!(host.agreed_l2_head_hash, B256::repeat_byte(0x33));
        assert_eq!(host.claimed_l2_output_root, B256::repeat_byte(0x44));
        assert_eq!(host.claimed_l2_block_number, 200);
        assert_eq!(host.l2_chain_id, Some(420));
        assert_eq!(host.l1_node_address, Some("http://l1:8545".to_string()));
        assert_eq!(
            host.rollup_config_path,
            Some(PathBuf::from("/tmp/rollup.json"))
        );
        assert_eq!(
            host.l1_config_path,
            Some(PathBuf::from("/tmp/l1-config.json"))
        );
    }

    /// Integration test — requires running L1/L2/Beacon/OP-Node.
    /// Run with: cargo test -p open-zk-host --features kona -- --ignored
    #[tokio::test]
    #[ignore]
    async fn rpc_witness_generation_integration() {
        use open_zk_core::types::ProvingMode;

        let provider = RpcWitnessProvider::new(
            "http://localhost:8545".to_string(),
            "http://localhost:9545".to_string(),
            "http://localhost:5052".to_string(),
        )
        .with_op_node_url("http://localhost:7545".to_string());

        let request = ProofRequest {
            l1_head: B256::ZERO,
            l2_start_block: 1,
            l2_end_block: 10,
            l2_start_output_root: B256::ZERO,
            mode: ProvingMode::Groth16,
        };

        let witness = provider.generate_witness(&request).await.unwrap();
        assert!(!witness.boot_info.is_empty());
        assert!(!witness.oracle_data.is_empty());
    }
}
