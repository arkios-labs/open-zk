//! Integration tests for ProofDispatcher and engine dispatch logic.

use alloy_primitives::B256;
use open_zk_core::types::{ProofMode, ProvingMode, ZkvmBackend};
use open_zk_orchestrator::mock_monitor::MockMonitor;
use open_zk_orchestrator::{
    ChainState, EngineConfig, EngineEvent, MockDispatcher, OrchestrationEngine, ProofDispatcher,
    ProofJobStatus,
};
use std::time::{Duration, SystemTime};

fn test_state() -> ChainState {
    ChainState {
        l1_head: B256::ZERO,
        l1_block_number: 100,
        l2_proven_block: 500,
        l2_safe_block: 750,
        timestamp: SystemTime::now(),
    }
}

fn beacon_intent() -> open_zk_orchestrator::ResolvedIntent {
    open_zk_orchestrator::ResolvedIntent {
        proof_mode: ProofMode::Beacon,
        backend: ZkvmBackend::Sp1,
        proving_mode: ProvingMode::Groth16,
        aggregation_window: 100,
    }
}

/// Beacon dispatch: submit range proofs via MockDispatcher and collect events.
#[tokio::test]
async fn beacon_dispatch_emits_correct_events() {
    let state = test_state();
    let monitor = MockMonitor {
        state: state.clone(),
    };
    let config = EngineConfig {
        intent: beacon_intent(),
        poll_interval: Duration::from_secs(10),
        max_concurrent_proofs: 4,
    };
    let mut engine = OrchestrationEngine::new(config, monitor, MockDispatcher);
    let mut rx = engine.take_event_receiver();

    // Run the engine in a task and cancel after collecting events
    let handle = tokio::spawn(async move {
        let _ = engine.run().await;
    });

    // Collect events with timeout
    let mut events = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        tokio::select! {
            Some(event) = rx.recv() => {
                events.push(event);
                // After getting aggregation completed, we have enough events
                if events.iter().any(|e| matches!(e, EngineEvent::AggregationCompleted { .. })) {
                    break;
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                break;
            }
        }
    }

    handle.abort();

    // Verify we got the expected event sequence
    assert!(events.iter().any(|e| matches!(
        e,
        EngineEvent::RangeDetected {
            start: 501,
            end: 750
        }
    )));

    // Should have 3 ProofStarted events (501-600, 601-700, 701-750)
    let proof_started_count = events
        .iter()
        .filter(|e| matches!(e, EngineEvent::ProofStarted { .. }))
        .count();
    assert_eq!(proof_started_count, 3);

    // Should have 3 ProofCompleted events
    let proof_completed_count = events
        .iter()
        .filter(|e| matches!(e, EngineEvent::ProofCompleted { .. }))
        .count();
    assert_eq!(proof_completed_count, 3);

    // Should have aggregation events (3 range proofs > 1)
    assert!(events
        .iter()
        .any(|e| matches!(e, EngineEvent::AggregationStarted { num_proofs: 3 })));
    assert!(events.iter().any(|e| matches!(
        e,
        EngineEvent::AggregationCompleted {
            start: 501,
            end: 750
        }
    )));
}

/// When there's no pending range, the engine emits no events.
#[tokio::test]
async fn no_pending_range_skips_dispatch() {
    let mut state = test_state();
    state.l2_safe_block = state.l2_proven_block; // caught up

    let monitor = MockMonitor { state };
    let config = EngineConfig {
        intent: beacon_intent(),
        poll_interval: Duration::from_millis(50),
        max_concurrent_proofs: 4,
    };
    let mut engine = OrchestrationEngine::new(config, monitor, MockDispatcher);
    let mut rx = engine.take_event_receiver();

    let handle = tokio::spawn(async move {
        let _ = engine.run().await;
    });

    // Wait a bit and check no events
    tokio::time::sleep(Duration::from_millis(200)).await;
    handle.abort();

    let event = rx.try_recv();
    assert!(event.is_err()); // No events emitted
}

/// Single range (window >= range size) skips aggregation.
#[tokio::test]
async fn single_range_skips_aggregation() {
    let mut state = test_state();
    state.l2_proven_block = 500;
    state.l2_safe_block = 550; // 50 blocks, fits in window=100

    let monitor = MockMonitor {
        state: state.clone(),
    };
    let config = EngineConfig {
        intent: beacon_intent(),
        poll_interval: Duration::from_secs(10),
        max_concurrent_proofs: 4,
    };
    let mut engine = OrchestrationEngine::new(config, monitor, MockDispatcher);
    let mut rx = engine.take_event_receiver();

    let handle = tokio::spawn(async move {
        let _ = engine.run().await;
    });

    let mut events = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        tokio::select! {
            Some(event) = rx.recv() => {
                events.push(event);
                if events.iter().any(|e| matches!(e, EngineEvent::ProofCompleted { .. })) {
                    break;
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                break;
            }
        }
    }

    handle.abort();

    // Only 1 range proof, no aggregation
    let agg_count = events
        .iter()
        .filter(|e| matches!(e, EngineEvent::AggregationStarted { .. }))
        .count();
    assert_eq!(agg_count, 0);
}

/// MockDispatcher submit and wait roundtrip.
#[tokio::test]
async fn mock_dispatcher_roundtrip() {
    let dispatcher = MockDispatcher;
    let request = open_zk_core::types::ProofRequest {
        l1_head: B256::ZERO,
        l2_start_block: 100,
        l2_end_block: 200,
        l2_start_output_root: B256::ZERO,
        mode: ProvingMode::Groth16,
    };

    let handle = dispatcher.submit(request).await.unwrap();
    assert_eq!(handle.id, "mock-100-200");

    let status = dispatcher.status(&handle).await.unwrap();
    assert!(matches!(status, ProofJobStatus::Completed(_)));

    let proof = dispatcher.wait(&handle).await.unwrap();
    assert_eq!(proof.backend, ZkvmBackend::Mock);
    assert_eq!(proof.proof_bytes, vec![0xDE, 0xAD]);
}
