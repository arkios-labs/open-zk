//! Mock end-to-end integration tests for the full proving pipeline.
//!
//! Exercises: OpenZkConfig → IntentResolver → OrchestrationEngine →
//!            ProverBackend → ProofArtifact without real zkVM backends
//!            or RPC connections.

use alloy_primitives::B256;
use open_zk::core::traits::{ProverBackend, WitnessProvider};
use open_zk::core::types::{
    BootInfo, ProofMode, ProvingMode, SecurityLevel, StateTransitionJournal, ZkvmBackend,
};
use open_zk::OpenZkConfig;
use open_zk_contracts::client::{MockProofSubmitter, ProofSubmitter};
use open_zk_host::prover::{MockProgram, MockProverBackend, MockWitness};
use open_zk_host::witness::MockWitnessProvider;
use open_zk_orchestrator::mock_monitor::MockMonitor;
use open_zk_orchestrator::{
    ChainMonitor, ChainState, EngineConfig, EngineEvent, MockDispatcher, OrchestrationEngine,
    ProofDispatcher,
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

/// Full happy path: Config → resolve → MockMonitor → pending_range →
/// plan_range → MockWitnessProvider → MockProverBackend::prove → verify.
#[tokio::test]
async fn test_full_pipeline_config_to_proof() {
    // Step 1: Build config and resolve intent
    let config = OpenZkConfig::builder()
        .target_finality(Duration::from_secs(600)) // 10 min → Beacon
        .security(SecurityLevel::Standard)
        .l1_rpc_url("http://localhost:8545")
        .l2_rpc_url("http://localhost:9545")
        .l1_beacon_url("http://localhost:5052")
        .build()
        .unwrap();

    let intent = config.resolve();
    assert_eq!(intent.proof_mode, ProofMode::Beacon);
    assert_eq!(intent.backend, ZkvmBackend::Sp1);
    assert_eq!(intent.aggregation_window, 100);

    // Step 2: Set up orchestration engine with mock monitor
    let state = test_state();
    let monitor = MockMonitor {
        state: state.clone(),
    };
    let engine_config = EngineConfig {
        intent,
        poll_interval: Duration::from_secs(10),
        max_concurrent_proofs: 4,
    };
    let engine = OrchestrationEngine::new(engine_config, monitor, MockDispatcher);

    // Step 3: Get pending range and plan
    let pending_monitor = MockMonitor {
        state: state.clone(),
    };
    let range = pending_monitor.pending_range().await.unwrap();
    assert_eq!(range, Some((501, 750)));

    let (start, end) = range.unwrap();
    let requests = engine.plan_range(start, end, &state);
    assert_eq!(requests.len(), 3); // 501-600, 601-700, 701-750

    // Step 4: Generate witness for each request
    let witness_provider = MockWitnessProvider;
    for request in &requests {
        let witness = witness_provider.generate_witness(request).await.unwrap();
        assert!(!witness.boot_info.is_empty());
        assert!(!witness.oracle_data.is_empty());
        assert!(!witness.blob_data.is_empty());
    }

    // Step 5: Prove and verify with mock backend
    let prover = MockProverBackend;
    let program = MockProgram::new("test-range");
    let witness = MockWitness::default();

    let proof = prover
        .prove(&program, &witness, ProvingMode::Groth16)
        .await
        .unwrap();
    assert_eq!(proof.backend, ZkvmBackend::Mock);
    assert_eq!(proof.proof_bytes, vec![0xDE, 0xAD]);

    let valid = prover.verify(&program, &proof).await.unwrap();
    assert!(valid);
}

/// JSON serialize → deserialize of StateTransitionJournal.
#[test]
fn test_journal_serde_roundtrip() {
    let journal = StateTransitionJournal {
        l1_head: B256::repeat_byte(0xAA),
        l2_pre_root: B256::repeat_byte(0xBB),
        l2_post_root: B256::repeat_byte(0xCC),
        l2_block_number: 12345,
        rollup_config_hash: B256::repeat_byte(0xDD),
        program_id: B256::repeat_byte(0xEE),
    };

    let json = serde_json::to_string(&journal).unwrap();
    let decoded: StateTransitionJournal = serde_json::from_str(&json).unwrap();
    assert_eq!(journal, decoded);
}

/// to_abi_bytes() → from_abi_bytes() roundtrip.
#[test]
fn test_journal_abi_roundtrip() {
    let journal = StateTransitionJournal {
        l1_head: B256::repeat_byte(0x01),
        l2_pre_root: B256::repeat_byte(0x02),
        l2_post_root: B256::repeat_byte(0x03),
        l2_block_number: 42,
        rollup_config_hash: B256::repeat_byte(0x04),
        program_id: B256::repeat_byte(0x05),
    };

    let bytes = journal.to_abi_bytes();
    assert!(!bytes.is_empty());
    assert_eq!(bytes.len(), 192); // 6 ABI slots × 32 bytes
    let decoded = StateTransitionJournal::from_abi_bytes(&bytes).unwrap();
    assert_eq!(journal, decoded);
}

/// Edge case: all-zero journal still round-trips through ABI encoding.
#[test]
fn test_journal_abi_with_zero_values() {
    let journal = StateTransitionJournal {
        l1_head: B256::ZERO,
        l2_pre_root: B256::ZERO,
        l2_post_root: B256::ZERO,
        l2_block_number: 0,
        rollup_config_hash: B256::ZERO,
        program_id: B256::ZERO,
    };

    let bytes = journal.to_abi_bytes();
    assert_eq!(bytes.len(), 192);
    let decoded = StateTransitionJournal::from_abi_bytes(&bytes).unwrap();
    assert_eq!(journal, decoded);
}

/// When l2_safe_block == l2_proven_block, pending_range() returns None.
#[tokio::test]
async fn test_no_pending_blocks_produces_no_requests() {
    let mut state = test_state();
    state.l2_safe_block = state.l2_proven_block; // caught up

    let monitor = MockMonitor { state };
    let range = monitor.pending_range().await.unwrap();
    assert_eq!(range, None);
}

/// Economy security → Sentinel + RiscZero + window=1000.
/// A 250-block range fits in a single request.
#[tokio::test]
async fn test_economy_config_resolves_sentinel() {
    let config = OpenZkConfig::builder()
        .target_finality(Duration::from_secs(3600))
        .security(SecurityLevel::Economy)
        .l1_rpc_url("http://localhost:8545")
        .l2_rpc_url("http://localhost:9545")
        .l1_beacon_url("http://localhost:5052")
        .build()
        .unwrap();

    let intent = config.resolve();
    assert_eq!(intent.proof_mode, ProofMode::Sentinel);
    assert_eq!(intent.backend, ZkvmBackend::RiscZero);
    assert_eq!(intent.aggregation_window, 1000);

    // With a 250-block range and window=1000, we get a single request
    let state = test_state(); // l2_proven=500, l2_safe=750 → range 501..750
    let monitor = MockMonitor {
        state: state.clone(),
    };
    let engine_config = EngineConfig {
        intent,
        poll_interval: Duration::from_secs(60),
        max_concurrent_proofs: 1,
    };
    let engine = OrchestrationEngine::new(engine_config, monitor, MockDispatcher);

    let requests = engine.plan_range(501, 750, &state);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].l2_start_block, 501);
    assert_eq!(requests[0].l2_end_block, 750);
}

/// E2E: Config → resolve → engine → dispatcher → proof → contract submitter.
#[tokio::test]
async fn test_full_pipeline_with_dispatcher_and_submitter() {
    // Step 1: Config
    let config = OpenZkConfig::builder()
        .target_finality(Duration::from_secs(600))
        .security(SecurityLevel::Standard)
        .l1_rpc_url("http://localhost:8545")
        .l2_rpc_url("http://localhost:9545")
        .l1_beacon_url("http://localhost:5052")
        .build()
        .unwrap();

    let intent = config.resolve();
    let state = test_state();

    // Step 2: Engine with dispatcher
    let monitor = MockMonitor {
        state: state.clone(),
    };
    let engine_config = EngineConfig {
        intent,
        poll_interval: Duration::from_secs(10),
        max_concurrent_proofs: 4,
    };
    let engine = OrchestrationEngine::new(engine_config, monitor, MockDispatcher);

    // Step 3: Plan range
    let requests = engine.plan_range(501, 750, &state);
    assert_eq!(requests.len(), 3);

    // Step 4: Dispatch via MockDispatcher
    let dispatcher = MockDispatcher;
    for request in &requests {
        let handle = dispatcher.submit(request.clone()).await.unwrap();
        let proof = dispatcher.wait(&handle).await.unwrap();
        assert_eq!(proof.backend, ZkvmBackend::Mock);

        // Step 5: Submit via MockProofSubmitter
        let journal = StateTransitionJournal {
            l1_head: request.l1_head,
            l2_pre_root: request.l2_start_output_root,
            l2_post_root: B256::repeat_byte(0xFF),
            l2_block_number: request.l2_end_block,
            rollup_config_hash: B256::ZERO,
            program_id: B256::ZERO,
        };

        let submitter = MockProofSubmitter;
        let tx_hash = submitter.submit_proof(&journal, &proof).await.unwrap();
        assert_eq!(tx_hash, B256::ZERO);
    }
}

/// E2E: Engine beacon loop emits correct event sequence via dispatcher.
#[tokio::test]
async fn test_engine_beacon_loop_with_events() {
    let state = test_state();
    let monitor = MockMonitor {
        state: state.clone(),
    };
    let config = EngineConfig {
        intent: open_zk_orchestrator::ResolvedIntent {
            proof_mode: ProofMode::Beacon,
            backend: ZkvmBackend::Sp1,
            proving_mode: ProvingMode::Groth16,
            aggregation_window: 100,
        },
        poll_interval: Duration::from_secs(10),
        max_concurrent_proofs: 4,
    };

    let mut engine = OrchestrationEngine::new(config, monitor, MockDispatcher);
    let mut rx = engine.take_event_receiver();

    let handle = tokio::spawn(async move {
        let _ = engine.run().await;
    });

    // Collect events
    let mut events = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        tokio::select! {
            Some(event) = rx.recv() => {
                events.push(event);
                if events.iter().any(|e| matches!(e, EngineEvent::AggregationCompleted { .. })) {
                    break;
                }
            }
            _ = tokio::time::sleep_until(deadline) => { break; }
        }
    }
    handle.abort();

    // Verify event flow: RangeDetected → 3x(ProofStarted, ProofCompleted) → AggregationStarted → AggregationCompleted
    assert!(events
        .iter()
        .any(|e| matches!(e, EngineEvent::RangeDetected { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, EngineEvent::AggregationCompleted { .. })));

    let starts = events
        .iter()
        .filter(|e| matches!(e, EngineEvent::ProofStarted { .. }))
        .count();
    assert_eq!(starts, 3);
}

/// BootInfo ABI roundtrip in E2E context.
#[test]
fn test_boot_info_in_pipeline_context() {
    let boot = BootInfo {
        l1_head: B256::repeat_byte(0x01),
        l2_pre_root: B256::repeat_byte(0x02),
        l2_claim: B256::repeat_byte(0x03),
        l2_block_number: 750,
        rollup_config_hash: B256::repeat_byte(0x04),
    };

    let bytes = boot.to_abi_bytes();
    let decoded = BootInfo::from_abi_bytes(&bytes).unwrap();
    assert_eq!(boot, decoded);

    // Verify boot info maps to journal fields
    let journal = StateTransitionJournal {
        l1_head: decoded.l1_head,
        l2_pre_root: decoded.l2_pre_root,
        l2_post_root: decoded.l2_claim,
        l2_block_number: decoded.l2_block_number,
        rollup_config_hash: decoded.rollup_config_hash,
        program_id: B256::ZERO,
    };

    let journal_bytes = journal.to_abi_bytes();
    let journal_decoded = StateTransitionJournal::from_abi_bytes(&journal_bytes).unwrap();
    assert_eq!(journal, journal_decoded);
}
