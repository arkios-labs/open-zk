//! RPC-based witness provider using kona-host for preimage collection.
//!
//! Connects to L1 execution, L2 execution, and L1 beacon endpoints to
//! collect all preimages needed for the guest derivation pipeline.

use std::path::PathBuf;
use std::sync::Arc;

use alloy_primitives::B256;
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
use tracing::info;

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
        //
        // These values are needed to configure SingleChainHost:
        // - agreed_l2_head_hash: block hash at l2_start_block
        // - claimed_l2_output_root: output root at l2_end_block (derived from L2 node)
        //
        // TODO: Make actual RPC calls to fetch these values.
        // For now, these must be provided externally or fetched before calling.
        // In production, this would use alloy provider:
        //   let l2_provider = ProviderBuilder::new().on_http(l2_rpc_url);
        //   let start_block = l2_provider.get_block_by_number(l2_start_block).await?;
        //   let end_block = l2_provider.get_block_by_number(l2_end_block).await?;
        let agreed_l2_head_hash = B256::ZERO; // TODO: fetch from L2 RPC
        let claimed_l2_output_root = B256::ZERO; // TODO: fetch from L2 RPC

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
            rollup_config_hash: B256::ZERO, // TODO: compute from rollup config
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
