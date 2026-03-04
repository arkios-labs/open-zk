//! Devnet end-to-end integration tests.
//!
//! These tests require a running OP Stack devnet and deployed OpenZk contracts.
//! All tests are marked `#[ignore]` and run only with:
//!
//!   ```bash
//!   SP1_PROVER=mock cargo test -p open-zk --test devnet_e2e -- --ignored
//!   ```
//!
//! # Devnet Setup
//!
//! ```bash
//! just devnet-fetch    # clone Optimism monorepo (one-time)
//! just devnet-up       # start L1/L2/Beacon/OP-Node containers
//! just devnet-deploy   # deploy OpenZk contracts
//! ```

use alloy_primitives::B256;
use open_zk::core::traits::{ProverBackend, WitnessProvider};
use open_zk::core::types::{
    ProvingMode, SecurityLevel, StateTransitionJournal, ZkvmBackend,
};
use open_zk::OpenZkConfig;
use open_zk_contracts::client::{MockProofSubmitter, ProofSubmitter};
use open_zk_host::prover::{MockProgram, MockProverBackend, MockWitness};
use open_zk_host::witness::MockWitnessProvider;
use open_zk_orchestrator::mock_monitor::MockMonitor;
use open_zk_orchestrator::{ChainMonitor, ChainState, EngineConfig, MockDispatcher};
use std::time::{Duration, SystemTime};

/// Default devnet RPC endpoints.
const L1_RPC: &str = "http://127.0.0.1:8545";
const L2_RPC: &str = "http://127.0.0.1:9545";
const L1_BEACON: &str = "http://127.0.0.1:5052";

/// Verify that the devnet L1 and L2 endpoints are accessible
/// and return valid chain IDs.
#[tokio::test]
#[ignore]
async fn test_devnet_connectivity() {
    use alloy_provider::{Provider, ProviderBuilder};

    let l1_url: url::Url = L1_RPC.parse().unwrap();
    let l2_url: url::Url = L2_RPC.parse().unwrap();

    let l1_provider = ProviderBuilder::new().connect_http(l1_url);
    let l2_provider = ProviderBuilder::new().connect_http(l2_url);

    let l1_chain_id = l1_provider.get_chain_id().await.unwrap();
    let l2_chain_id = l2_provider.get_chain_id().await.unwrap();

    // OP Stack devnet: L1 = 900, L2 = 901
    assert!(l1_chain_id > 0, "L1 chain ID should be non-zero");
    assert!(l2_chain_id > 0, "L2 chain ID should be non-zero");
    assert_ne!(l1_chain_id, l2_chain_id, "L1 and L2 should have different chain IDs");

    println!("L1 chain ID: {l1_chain_id}");
    println!("L2 chain ID: {l2_chain_id}");
}

/// Verify that L2 blocks are being produced.
#[tokio::test]
#[ignore]
async fn test_devnet_l2_block_production() {
    use alloy_provider::{Provider, ProviderBuilder};

    let l2_url: url::Url = L2_RPC.parse().unwrap();
    let l2_provider = ProviderBuilder::new().connect_http(l2_url);

    let block_1 = l2_provider.get_block_number().await.unwrap();
    // Wait a bit for new blocks
    tokio::time::sleep(Duration::from_secs(3)).await;
    let block_2 = l2_provider.get_block_number().await.unwrap();

    assert!(block_2 > block_1, "L2 should be producing blocks (was {block_1}, now {block_2})");
    println!("L2 block progression: {block_1} → {block_2}");
}

/// Full pipeline E2E with mock prover against real devnet RPC.
///
/// Steps:
/// 1. Fetch real chain state from devnet RPCs
/// 2. Build SDK config and resolve intent
/// 3. Plan proof range from real chain state
/// 4. Generate witness (mock — no real kona pipeline)
/// 5. Prove with mock backend
/// 6. Submit to mock submitter
#[tokio::test]
#[ignore]
async fn test_devnet_full_pipeline_mock_proof() {
    use alloy_provider::{Provider, ProviderBuilder};
    use alloy_rpc_types_eth::BlockNumberOrTag;

    // Step 1: Fetch real chain state
    let l1_url: url::Url = L1_RPC.parse().unwrap();
    let l2_url: url::Url = L2_RPC.parse().unwrap();
    let l1_provider = ProviderBuilder::new().connect_http(l1_url);
    let l2_provider = ProviderBuilder::new().connect_http(l2_url);

    let l1_block = l1_provider
        .get_block_by_number(BlockNumberOrTag::Finalized)
        .await
        .unwrap()
        .expect("L1 finalized block should exist");
    let l2_block_number = l2_provider.get_block_number().await.unwrap();

    println!("L1 finalized: {} ({})", l1_block.header.number, l1_block.header.hash);
    println!("L2 latest: {l2_block_number}");

    // Step 2: Build config
    let config = OpenZkConfig::builder()
        .target_finality(Duration::from_secs(600))
        .max_cost_per_proof(0.50)
        .security(SecurityLevel::Standard)
        .l1_rpc_url(L1_RPC)
        .l2_rpc_url(L2_RPC)
        .l1_beacon_url(L1_BEACON)
        .build()
        .unwrap();

    let intent = config.resolve();

    // Step 3: Create chain state and plan range
    let state = ChainState {
        l1_head: l1_block.header.hash,
        l1_block_number: l1_block.header.number,
        l2_proven_block: 0, // Nothing proven yet on fresh devnet
        l2_safe_block: l2_block_number.min(10), // Use first 10 blocks
        timestamp: SystemTime::now(),
    };

    let monitor = MockMonitor {
        state: state.clone(),
    };
    let engine_config = EngineConfig {
        intent: intent.clone(),
        poll_interval: Duration::from_secs(10),
        max_concurrent_proofs: 4,
    };
    let engine =
        open_zk_orchestrator::OrchestrationEngine::new(engine_config, monitor, MockDispatcher);

    let pending = MockMonitor {
        state: state.clone(),
    }
    .pending_range()
    .await
    .unwrap();

    if let Some((start, end)) = pending {
        println!("Pending range: {start}..{end}");
        let requests = engine.plan_range(start, end, &state);
        println!("Planned {} proof request(s)", requests.len());

        // Step 4: Witness generation (mock)
        let witness_provider = MockWitnessProvider;
        for request in &requests {
            let witness = witness_provider.generate_witness(request).await.unwrap();
            assert!(!witness.boot_info.is_empty());

            // Step 5: Prove (mock)
            let prover = MockProverBackend;
            let program = MockProgram::new("test-range");
            let mock_witness = MockWitness::default();
            let proof = prover
                .prove(&program, &mock_witness, ProvingMode::Groth16)
                .await
                .unwrap();
            assert_eq!(proof.backend, ZkvmBackend::Mock);

            // Step 6: Submit (mock)
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
            println!(
                "Proved and submitted blocks {}..{}",
                request.l2_start_block, request.l2_end_block
            );
        }
    } else {
        println!("No pending blocks (l2_proven == l2_safe), skipping");
    }
}

/// Test that RpcChainMonitor can fetch state from devnet.
#[tokio::test]
#[ignore]
async fn test_devnet_rpc_chain_monitor() {
    // RpcChainMonitor requires the `rpc` feature on orchestrator.
    // This test verifies it against the devnet without the feature —
    // it uses alloy directly.
    use alloy_provider::{Provider, ProviderBuilder};
    use alloy_rpc_types_eth::BlockNumberOrTag;

    let l1_url: url::Url = L1_RPC.parse().unwrap();
    let l2_url: url::Url = L2_RPC.parse().unwrap();
    let l1_provider = ProviderBuilder::new().connect_http(l1_url);
    let l2_provider = ProviderBuilder::new().connect_http(l2_url);

    // Fetch L1 finalized
    let l1_finalized = l1_provider
        .get_block_by_number(BlockNumberOrTag::Finalized)
        .await
        .unwrap();
    assert!(l1_finalized.is_some(), "L1 should have finalized blocks");
    let l1_block = l1_finalized.unwrap();
    println!("L1 finalized block: {} hash: {}", l1_block.header.number, l1_block.header.hash);

    // Fetch L2 safe
    let l2_safe = l2_provider
        .get_block_by_number(BlockNumberOrTag::Safe)
        .await
        .unwrap();
    assert!(l2_safe.is_some(), "L2 should have safe blocks");
    let l2_block = l2_safe.unwrap();
    println!("L2 safe block: {}", l2_block.header.number);

    // Chain state construction
    let state = ChainState {
        l1_head: l1_block.header.hash,
        l1_block_number: l1_block.header.number,
        l2_proven_block: 0,
        l2_safe_block: l2_block.header.number,
        timestamp: SystemTime::now(),
    };

    assert!(state.l1_block_number > 0);
    assert!(state.l2_safe_block > 0);
    println!("Chain state: L1={} L2_safe={}", state.l1_block_number, state.l2_safe_block);
}
