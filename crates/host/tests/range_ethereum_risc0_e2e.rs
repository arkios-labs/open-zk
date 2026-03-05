//! E2E test for the range-ethereum guest program on RISC Zero.
//!
//! Full pipeline: devnet → witness generation → RISC Zero dev mode execution → journal verification.
//!
//! Prerequisites:
//!   - Running OP Stack devnet (`just devnet-up`)
//!   - RISC Zero ELF built:
//!     cargo build -p open-zk-build-risc0 --features rebuild-guest,debug-guest-build
//!
//! Run:
//!   RISC0_DEV_MODE=1 cargo test -p open-zk-host --features "risc0,kona" \
//!     --test range_ethereum_risc0_e2e -- --ignored --nocapture

#![cfg(all(feature = "risc0", feature = "kona"))]

use alloy_primitives::B256;
use open_zk_core::traits::{ProverBackend, WitnessProvider};
use open_zk_core::types::{ProofRequest, ProvingMode, StateTransitionJournal};
use open_zk_host::prover::{RiscZeroProverBackend, RiscZeroProgram, RiscZeroWitness};
use open_zk_host::witness::RpcWitnessProvider;
use std::time::Instant;

const L1_RPC: &str = "http://127.0.0.1:8545";
const L2_RPC: &str = "http://127.0.0.1:9545";
const L1_BEACON: &str = "http://127.0.0.1:5052";
const OP_NODE: &str = "http://127.0.0.1:7545";

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
}

async fn get_l1_head() -> B256 {
    use alloy_provider::{Provider, ProviderBuilder};
    use alloy_rpc_types_eth::BlockNumberOrTag;

    let url: url::Url = L1_RPC.parse().unwrap();
    let provider = ProviderBuilder::new().connect_http(url);
    let block = provider
        .get_block_by_number(BlockNumberOrTag::Latest)
        .await
        .unwrap()
        .unwrap();
    block.header.hash
}

async fn get_l2_output_root(block_number: u64) -> B256 {
    use alloy_provider::{Provider, ProviderBuilder};

    let url: url::Url = OP_NODE.parse().unwrap();
    let provider = ProviderBuilder::new().connect_http(url);
    let resp: serde_json::Value = provider
        .raw_request(
            "optimism_outputAtBlock".into(),
            [format!("0x{:x}", block_number)],
        )
        .await
        .unwrap();
    resp["outputRoot"]
        .as_str()
        .unwrap()
        .parse::<B256>()
        .unwrap()
}

/// Full E2E: witness generation → RISC Zero execute (dev mode) → journal verification.
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn test_range_ethereum_risc0_e2e_devnet() {
    init_tracing();

    // Step 1: Get devnet state
    let l1_head = get_l1_head().await;
    let l2_start_output_root = get_l2_output_root(1).await;
    println!("L1 head: {l1_head}");
    println!("L2 output root at block 1: {l2_start_output_root}");

    // Step 2: Generate witness
    let provider = RpcWitnessProvider::new(
        L1_RPC.to_string(),
        L2_RPC.to_string(),
        L1_BEACON.to_string(),
    )
    .with_op_node_url(OP_NODE.to_string())
    .with_chain_id(901);

    let request = ProofRequest {
        l1_head,
        l2_start_block: 1,
        l2_end_block: 2,
        l2_start_output_root,
        mode: ProvingMode::Execute,
    };

    println!("Generating witness for blocks 1→2...");
    let start = Instant::now();
    let witness = provider
        .generate_witness(&request)
        .await
        .expect("witness generation failed");
    println!(
        "Witness generated in {:.2}s ({} bytes oracle data)",
        start.elapsed().as_secs_f64(),
        witness.oracle_data.len()
    );

    // Step 3: Build RISC Zero witness (single oracle_data Vec<u8>)
    let rz_witness = RiscZeroWitness {
        oracle_data: witness.oracle_data,
    };

    // Step 4: Load the range-ethereum ELF (built by risc0-build)
    let elf = open_zk_host::elf::risc0::GUEST_RANGE_ETHEREUM_ELF;
    let image_id = open_zk_host::elf::risc0::GUEST_RANGE_ETHEREUM_ID;
    let program = RiscZeroProgram::new("range-ethereum", image_id, elf.to_vec());

    // Step 5: Execute in dev mode
    println!("Executing range-ethereum ELF in RISC Zero dev mode...");
    let start = Instant::now();
    let backend = RiscZeroProverBackend::new();
    let result = backend
        .prove(&program, &rz_witness, ProvingMode::Execute)
        .await
        .expect("RISC Zero execute failed");
    println!(
        "Execution completed in {:.2}s",
        start.elapsed().as_secs_f64()
    );
    println!("  cycle_count: {:?}", result.cycle_count);
    println!("  public_values: {} bytes", result.public_values.len());

    // Step 6: Verify journal
    assert!(
        !result.public_values.is_empty(),
        "public_values should not be empty"
    );

    let journal = StateTransitionJournal::from_abi_bytes(&result.public_values)
        .expect("failed to decode journal from public values");

    println!("Journal:");
    println!("  l1_head:          {}", journal.l1_head);
    println!("  l2_pre_root:      {}", journal.l2_pre_root);
    println!("  l2_post_root:     {}", journal.l2_post_root);
    println!("  l2_block_number:  {}", journal.l2_block_number);
    println!("  rollup_config_hash: {}", journal.rollup_config_hash);

    assert_eq!(journal.l1_head, l1_head, "l1_head mismatch");
    assert_eq!(
        journal.l2_pre_root, l2_start_output_root,
        "l2_pre_root mismatch"
    );
    assert_eq!(journal.l2_block_number, 2, "l2_block_number mismatch");

    let expected_post_root = get_l2_output_root(2).await;
    assert_eq!(
        journal.l2_post_root, expected_post_root,
        "l2_post_root mismatch with op-node"
    );

    println!("E2E test PASSED — range-ethereum guest correctly derived L2 state transition on RISC Zero");
}
