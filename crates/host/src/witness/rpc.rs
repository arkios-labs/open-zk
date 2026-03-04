//! RPC-based witness provider using kona-host for preimage collection.
//!
//! Connects to L1 execution, L2 execution, and L1 beacon endpoints to
//! collect all preimages needed for the guest derivation pipeline.

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
use tracing::{debug, info};

use super::kv_store::{serialize_preimages, ArcMemoryKvStore};

/// RPC-based witness provider that fetches L1/L2 data via kona-host.
///
/// Runs the kona fault proof program natively to discover all required
/// preimages, then serializes them into a [`RawWitness`] for offline proving.
pub struct RpcWitnessProvider {
    l1_rpc_url: String,
    l2_rpc_url: String,
    l1_beacon_url: String,
    /// Optional rollup config path. If None, fetched from L2 node.
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
            rollup_config_path: None,
            l2_chain_id: None,
        }
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
            rollup_config_path: self.rollup_config_path.clone(),
            data_dir: None, // Use in-memory KV store
            ..Default::default()
        }
    }

    /// Compute a hash of the rollup config for the guest BootInfo.
    ///
    /// If a rollup config path is provided, reads and hashes its JSON contents.
    /// Otherwise returns `B256::ZERO` (rollup config fetched at runtime).
    fn compute_rollup_config_hash(&self) -> Result<B256, RpcWitnessError> {
        match &self.rollup_config_path {
            Some(path) => {
                let contents = std::fs::read(path).map_err(|e| {
                    RpcWitnessError::Fetch(format!(
                        "failed to read rollup config at {}: {e}",
                        path.display()
                    ))
                })?;
                // Normalize JSON: parse and re-serialize to get canonical form
                let value: serde_json::Value =
                    serde_json::from_slice(&contents).map_err(|e| {
                        RpcWitnessError::Fetch(format!("invalid rollup config JSON: {e}"))
                    })?;
                let canonical =
                    serde_json::to_vec(&value).map_err(|e| {
                        RpcWitnessError::Serialization(format!(
                            "rollup config serialization: {e}"
                        ))
                    })?;
                Ok(keccak256(&canonical))
            }
            None => {
                // No static rollup config — hash will be determined at runtime
                debug!("no rollup config path set, using zero hash");
                Ok(B256::ZERO)
            }
        }
    }

    /// Fetch the L2 output root at a given block number.
    ///
    /// Tries `optimism_outputAtBlock` RPC first. If that fails, falls back to
    /// computing the output root from block header fields:
    /// `output_root = keccak256(version ++ state_root ++ withdrawals_root ++ block_hash)`
    async fn fetch_output_root(
        &self,
        l2_provider: &impl Provider,
        block_number: u64,
    ) -> Result<B256, RpcWitnessError> {
        // Try optimism_outputAtBlock RPC
        let rpc_result: Result<serde_json::Value, _> = l2_provider
            .raw_request(
                "optimism_outputAtBlock".into(),
                [format!("0x{:x}", block_number)],
            )
            .await;

        if let Ok(resp) = rpc_result {
            if let Some(root_str) = resp.get("outputRoot").and_then(|v| v.as_str()) {
                if let Ok(root) = root_str.parse::<B256>() {
                    debug!(block = block_number, output_root = %root, "fetched output root via RPC");
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
    async fn collect_witness(
        &self,
        host: &SingleChainHost,
    ) -> Result<ArcMemoryKvStore, RpcWitnessError> {
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
            l2_start = host.claimed_l2_block_number,
            "starting kona witness collection"
        );

        // Run server and client concurrently
        let server_task = tokio::spawn(async move { server.start().await });
        let client_task = tokio::spawn(kona_client::single::run(
            OracleReader::new(preimage_channel.client),
            HintWriter::new(hint_channel.client),
        ));

        // Wait for the client to finish (server terminates when channels close)
        let (server_result, client_result) = tokio::join!(server_task, client_task);

        // Handle client result (the important one — did derivation succeed?)
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

        // Server errors are expected (IOError when client channel closes)
        if let Ok(Err(e)) = server_result {
            // PreimageServerError::IOError is expected when client finishes
            let err_str = e.to_string();
            if !err_str.contains("IO") {
                return Err(RpcWitnessError::KonaHost(err_str));
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
        // Pre-flight: fetch L2 head hash and claimed output root from L2 node.
        let l2_url: url::Url = self
            .l2_rpc_url
            .parse()
            .map_err(|e| RpcWitnessError::Connection(format!("invalid L2 URL: {e}")))?;
        let l2_provider = ProviderBuilder::new().connect_http(l2_url);

        // 1. Fetch block hash at l2_start_block (agreed L2 head)
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

        // 2. Fetch output root at l2_end_block.
        //    Try OP Stack's `optimism_outputAtBlock` RPC first, then fall back to
        //    computing output_root = keccak256(version ++ state_root ++ withdrawals_root ++ block_hash).
        let claimed_l2_output_root = self
            .fetch_output_root(&l2_provider, request.l2_end_block)
            .await?;

        debug!(
            block = request.l2_end_block,
            output_root = %claimed_l2_output_root,
            "determined L2 claimed output root"
        );

        // 3. Compute rollup config hash
        let rollup_config_hash = self.compute_rollup_config_hash()?;

        // Build kona host configuration
        let host = self.build_host_config(request, agreed_l2_head_hash, claimed_l2_output_root);

        // Run the witness collection pipeline
        let store = self.collect_witness(&host).await?;

        // Extract and serialize preimages
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
    }

    #[test]
    fn rpc_witness_provider_with_config() {
        let provider = RpcWitnessProvider::new(
            "http://l1:8545".to_string(),
            "http://l2:9545".to_string(),
            "http://beacon:5052".to_string(),
        )
        .with_rollup_config(PathBuf::from("/etc/rollup.json"))
        .with_chain_id(10);

        assert_eq!(
            provider.rollup_config_path,
            Some(PathBuf::from("/etc/rollup.json"))
        );
        assert_eq!(provider.l2_chain_id, Some(10));
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
        );

        assert_eq!(host.l1_head, B256::repeat_byte(0x11));
        assert_eq!(host.agreed_l2_output_root, B256::repeat_byte(0x22));
        assert_eq!(host.agreed_l2_head_hash, B256::repeat_byte(0x33));
        assert_eq!(host.claimed_l2_output_root, B256::repeat_byte(0x44));
        assert_eq!(host.claimed_l2_block_number, 200);
        assert_eq!(host.l2_chain_id, Some(420));
        assert_eq!(host.l1_node_address, Some("http://l1:8545".to_string()));
    }

    /// Integration test — requires running L1/L2/Beacon nodes.
    /// Run with: cargo test -p open-zk-host --features kona -- --ignored
    #[tokio::test]
    #[ignore]
    async fn rpc_witness_generation_integration() {
        use open_zk_core::types::ProvingMode;

        let provider = RpcWitnessProvider::new(
            "http://localhost:8545".to_string(),
            "http://localhost:9545".to_string(),
            "http://localhost:5052".to_string(),
        );

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
