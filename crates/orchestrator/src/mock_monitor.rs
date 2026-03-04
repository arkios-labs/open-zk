use crate::monitor::{ChainMonitor, ChainState};
use async_trait::async_trait;

/// Error type for the mock monitor.
#[derive(Debug, thiserror::Error)]
#[error("mock monitor error")]
pub struct MockMonitorError;

/// A mock implementation of [`ChainMonitor`] for testing.
///
/// Returns the injected [`ChainState`] on every call, enabling
/// deterministic integration tests without real RPC connections.
pub struct MockMonitor {
    pub state: ChainState,
}

#[async_trait]
impl ChainMonitor for MockMonitor {
    type Error = MockMonitorError;

    async fn get_state(&self) -> Result<ChainState, Self::Error> {
        Ok(self.state.clone())
    }
}
