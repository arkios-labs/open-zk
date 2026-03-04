use alloy_primitives::B256;
use async_trait::async_trait;
use std::time::SystemTime;

/// Snapshot of L1/L2 chain state at a point in time.
#[derive(Debug, Clone)]
pub struct ChainState {
    /// Latest finalized L1 block hash.
    pub l1_head: B256,
    /// Latest finalized L1 block number.
    pub l1_block_number: u64,
    /// Latest proven L2 block number (already submitted on-chain).
    pub l2_proven_block: u64,
    /// Latest safe L2 block number (derived from L1 but not yet proven).
    pub l2_safe_block: u64,
    /// Timestamp of this snapshot.
    pub timestamp: SystemTime,
}

/// Monitors L1/L2 chain state to determine what needs proving.
#[async_trait]
pub trait ChainMonitor: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Fetch the current chain state.
    async fn get_state(&self) -> Result<ChainState, Self::Error>;

    /// Returns the range of L2 blocks that need proving:
    /// (proven_block + 1) .. safe_block.
    async fn pending_range(&self) -> Result<Option<(u64, u64)>, Self::Error> {
        let state = self.get_state().await?;
        if state.l2_safe_block > state.l2_proven_block {
            Ok(Some((state.l2_proven_block + 1, state.l2_safe_block)))
        } else {
            Ok(None)
        }
    }
}
