//! RPC-based chain monitor for live L1/L2 monitoring.
//!
//! Connects to L1 and L2 execution clients to track the latest proven block,
//! safe block, and finalized L1 head. Behind the `rpc` feature flag.

use crate::engine::DisputeInfo;
use crate::monitor::{ChainMonitor, ChainState};
use async_trait::async_trait;

/// RPC-based chain monitor that fetches live state from L1/L2 nodes.
pub struct RpcChainMonitor {
    l1_rpc_url: String,
    l2_rpc_url: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RpcMonitorError {
    #[error("L1 RPC error: {0}")]
    L1Rpc(String),
    #[error("L2 RPC error: {0}")]
    L2Rpc(String),
}

impl RpcChainMonitor {
    pub fn new(l1_rpc_url: String, l2_rpc_url: String) -> Self {
        Self {
            l1_rpc_url,
            l2_rpc_url,
        }
    }
}

#[async_trait]
impl ChainMonitor for RpcChainMonitor {
    type Error = RpcMonitorError;

    async fn get_state(&self) -> Result<ChainState, Self::Error> {
        // TODO: Implement actual RPC calls:
        //
        // 1. Fetch latest finalized L1 block
        //    let l1_provider = ProviderBuilder::new().on_http(self.l1_rpc_url.parse().unwrap());
        //    let l1_block = l1_provider.get_block_by_number(BlockNumberOrTag::Finalized).await?;
        //
        // 2. Fetch latest proven L2 block from L2OutputOracle contract
        //    let oracle = IOpenZkL2OutputOracle::new(oracle_address, l1_provider);
        //    let proven_block = oracle.latestBlockNumber().call().await?;
        //
        // 3. Fetch latest safe L2 block
        //    let l2_provider = ProviderBuilder::new().on_http(self.l2_rpc_url.parse().unwrap());
        //    let safe_block = l2_provider.get_block_by_number(BlockNumberOrTag::Safe).await?;

        let _ = (&self.l1_rpc_url, &self.l2_rpc_url);

        Err(RpcMonitorError::L1Rpc(
            "RPC chain monitor not yet implemented — requires alloy provider integration"
                .to_string(),
        ))
    }

    async fn active_dispute(&self) -> Option<DisputeInfo> {
        // TODO: Query IOpenZkDisputeGame contract for active disputes
        // let dispute_game = IOpenZkDisputeGame::new(dispute_address, l1_provider);
        // Check for DisputeCreated events that haven't been resolved
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_monitor_creation() {
        let monitor = RpcChainMonitor::new(
            "http://localhost:8545".to_string(),
            "http://localhost:9545".to_string(),
        );
        assert_eq!(monitor.l1_rpc_url, "http://localhost:8545");
        assert_eq!(monitor.l2_rpc_url, "http://localhost:9545");
    }

    #[tokio::test]
    async fn rpc_monitor_returns_error_without_connection() {
        let monitor = RpcChainMonitor::new(
            "http://localhost:8545".to_string(),
            "http://localhost:9545".to_string(),
        );
        let result = monitor.get_state().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn rpc_monitor_no_active_dispute() {
        let monitor = RpcChainMonitor::new(
            "http://localhost:8545".to_string(),
            "http://localhost:9545".to_string(),
        );
        let dispute = monitor.active_dispute().await;
        assert!(dispute.is_none());
    }
}
