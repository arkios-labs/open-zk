use crate::intent::ResolvedIntent;
use crate::monitor::{ChainMonitor, ChainState};
use open_zk_core::types::{ProofMode, ProofRequest};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::info;

/// Configuration for the orchestration engine.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Resolved intent from user configuration.
    pub intent: ResolvedIntent,
    /// How often to poll for new blocks.
    pub poll_interval: Duration,
    /// Maximum number of concurrent proof jobs.
    pub max_concurrent_proofs: usize,
}

/// Events emitted by the orchestration engine.
#[derive(Debug, Clone)]
pub enum EngineEvent {
    /// New block range detected that needs proving.
    RangeDetected { start: u64, end: u64 },
    /// Proof generation started for a block range.
    ProofStarted { start: u64, end: u64 },
    /// Proof generation completed.
    ProofCompleted { start: u64, end: u64 },
    /// Proof submitted on-chain.
    ProofSubmitted { start: u64, end: u64 },
    /// Error during proving or submission.
    Error { message: String },
}

/// The main orchestration engine that drives the proving loop.
///
/// Operates in two modes based on the resolved intent:
/// - **Beacon**: continuously proves every block range and submits proofs.
/// - **Sentinel**: monitors for disputes and proves only when challenged.
pub struct OrchestrationEngine<M: ChainMonitor> {
    config: EngineConfig,
    monitor: M,
    event_tx: mpsc::UnboundedSender<EngineEvent>,
    event_rx: mpsc::UnboundedReceiver<EngineEvent>,
}

impl<M: ChainMonitor> OrchestrationEngine<M> {
    pub fn new(config: EngineConfig, monitor: M) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        Self {
            config,
            monitor,
            event_tx,
            event_rx,
        }
    }

    /// Returns a clone of the event sender for external listeners.
    pub fn event_sender(&self) -> mpsc::UnboundedSender<EngineEvent> {
        self.event_tx.clone()
    }

    /// Take ownership of the event receiver for consuming events.
    pub fn take_event_receiver(&mut self) -> mpsc::UnboundedReceiver<EngineEvent> {
        let (new_tx, new_rx) = mpsc::unbounded_channel();
        let old_rx = std::mem::replace(&mut self.event_rx, new_rx);
        self.event_tx = new_tx;
        old_rx
    }

    /// Run the engine loop. Blocks until cancelled.
    pub async fn run(&self) -> Result<(), M::Error> {
        match self.config.intent.proof_mode {
            ProofMode::Beacon => self.run_beacon_loop().await,
            ProofMode::Sentinel => self.run_sentinel_loop().await,
        }
    }

    /// Beacon mode: continuously prove every new block range.
    async fn run_beacon_loop(&self) -> Result<(), M::Error> {
        info!("starting beacon mode loop");

        loop {
            match self.monitor.pending_range().await? {
                Some((start, end)) => {
                    let _ = self.event_tx.send(EngineEvent::RangeDetected { start, end });

                    // Split range into aggregation windows
                    let window = self.config.intent.aggregation_window;
                    let mut cursor = start;

                    while cursor <= end {
                        let range_end = (cursor + window - 1).min(end);
                        let _ = self.event_tx.send(EngineEvent::ProofStarted {
                            start: cursor,
                            end: range_end,
                        });

                        // Phase 4 TODO: dispatch actual proof generation
                        // via ProverBackend::prove() with proper witness

                        let _ = self.event_tx.send(EngineEvent::ProofCompleted {
                            start: cursor,
                            end: range_end,
                        });

                        cursor = range_end + 1;
                    }
                }
                None => {
                    // No new blocks to prove, wait for next poll
                }
            }

            tokio::time::sleep(self.config.poll_interval).await;
        }
    }

    /// Sentinel mode: watch for disputes and prove on demand.
    async fn run_sentinel_loop(&self) -> Result<(), M::Error> {
        info!("starting sentinel mode loop");

        loop {
            // Phase 4 TODO: monitor dispute game contract for challenges
            // When a dispute is detected:
            // 1. Determine the disputed block range
            // 2. Generate proof for that range
            // 3. Submit proof to resolve the dispute

            tokio::time::sleep(self.config.poll_interval).await;
        }
    }

    /// Generate proof requests for a given block range, splitting
    /// into sub-ranges based on the aggregation window.
    pub fn plan_range(&self, start: u64, end: u64, state: &ChainState) -> Vec<ProofRequest> {
        let window = self.config.intent.aggregation_window;
        let mode = self.config.intent.proving_mode;
        let mut requests = Vec::new();
        let mut cursor = start;

        while cursor <= end {
            let range_end = (cursor + window - 1).min(end);
            requests.push(ProofRequest {
                l1_head: state.l1_head,
                l2_start_block: cursor,
                l2_end_block: range_end,
                l2_start_output_root: alloy_primitives::B256::ZERO, // filled by witness provider
                mode,
            });
            cursor = range_end + 1;
        }

        requests
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::ChainState;
    use alloy_primitives::B256;
    use open_zk_core::types::{ProofMode, ProvingMode, ZkvmBackend};
    use std::time::SystemTime;

    fn test_state() -> ChainState {
        ChainState {
            l1_head: B256::ZERO,
            l1_block_number: 100,
            l2_proven_block: 500,
            l2_safe_block: 750,
            timestamp: SystemTime::now(),
        }
    }

    fn beacon_config() -> EngineConfig {
        EngineConfig {
            intent: ResolvedIntent {
                proof_mode: ProofMode::Beacon,
                backend: ZkvmBackend::Sp1,
                proving_mode: ProvingMode::Groth16,
                aggregation_window: 100,
            },
            poll_interval: Duration::from_secs(10),
            max_concurrent_proofs: 4,
        }
    }

    struct MockMonitor {
        state: ChainState,
    }

    #[derive(Debug, thiserror::Error)]
    #[error("mock monitor error")]
    struct MockMonitorError;

    #[async_trait::async_trait]
    impl ChainMonitor for MockMonitor {
        type Error = MockMonitorError;

        async fn get_state(&self) -> Result<ChainState, Self::Error> {
            Ok(self.state.clone())
        }
    }

    #[test]
    fn plan_range_splits_by_aggregation_window() {
        let config = beacon_config();
        let monitor = MockMonitor {
            state: test_state(),
        };
        let engine = OrchestrationEngine::new(config, monitor);
        let state = test_state();

        let requests = engine.plan_range(501, 750, &state);
        // 501..600, 601..700, 701..750 = 3 ranges
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[0].l2_start_block, 501);
        assert_eq!(requests[0].l2_end_block, 600);
        assert_eq!(requests[1].l2_start_block, 601);
        assert_eq!(requests[1].l2_end_block, 700);
        assert_eq!(requests[2].l2_start_block, 701);
        assert_eq!(requests[2].l2_end_block, 750);
    }

    #[test]
    fn plan_range_single_window() {
        let mut config = beacon_config();
        config.intent.aggregation_window = 1000;
        let monitor = MockMonitor {
            state: test_state(),
        };
        let engine = OrchestrationEngine::new(config, monitor);
        let state = test_state();

        let requests = engine.plan_range(100, 200, &state);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].l2_start_block, 100);
        assert_eq!(requests[0].l2_end_block, 200);
    }

    #[tokio::test]
    async fn mock_monitor_pending_range() {
        let monitor = MockMonitor {
            state: test_state(),
        };
        let range = monitor.pending_range().await.unwrap();
        assert_eq!(range, Some((501, 750)));
    }

    #[tokio::test]
    async fn mock_monitor_no_pending_when_caught_up() {
        let mut state = test_state();
        state.l2_safe_block = state.l2_proven_block;
        let monitor = MockMonitor { state };
        let range = monitor.pending_range().await.unwrap();
        assert_eq!(range, None);
    }
}
