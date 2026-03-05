//! E2E tests for the range-ethereum guest program.
//!
//! Full pipeline: devnet → witness generation → zkVM execution → journal verification.
//!
//! Prerequisites:
//!   - Running OP Stack devnet (`just devnet-up`)
//!   - SP1 ELF built: `cd guests/range-ethereum && cargo prove build --features sp1`
//!   - RISC Zero ELF built:
//!     cargo build -p open-zk-build-risc0 --features rebuild-guest,debug-guest-build
//!
//! Run (one at a time, never in parallel):
//!   SP1_PROVER=mock cargo test -p open-zk-host --features "sp1,kona" \
//!     --test range_ethereum_e2e --release -- --ignored --nocapture
//!
//!   RISC0_DEV_MODE=1 cargo test -p open-zk-host --features "risc0,kona" \
//!     --test range_ethereum_e2e --release -- --ignored --nocapture

#![cfg(feature = "kona")]

mod common;

use common::{get_l1_head, get_l2_output_root, init_tracing, L1_BEACON, L1_RPC, L2_RPC, OP_NODE};
use open_zk_core::traits::{ProverBackend, WitnessProvider};
use open_zk_core::types::{ProofRequest, ProvingMode, StateTransitionJournal};
use open_zk_host::witness::RpcWitnessProvider;
use std::time::Instant;

async fn generate_witness() -> open_zk_core::traits::RawWitness {
    let l1_head = get_l1_head().await;
    let l2_start_output_root = get_l2_output_root(1).await;
    println!("L1 head: {l1_head}");
    println!("L2 output root at block 1: {l2_start_output_root}");

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
    witness
}

async fn verify_journal(result: &open_zk_core::types::ProofArtifact) {
    let l1_head = get_l1_head().await;
    let l2_start_output_root = get_l2_output_root(1).await;

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
}

#[cfg(feature = "sp1")]
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn test_range_ethereum_sp1_e2e_devnet() {
    use open_zk_host::prover::{Sp1ProverBackend, Sp1Program, Sp1Witness};
    use sp1_sdk::SP1Stdin;

    init_tracing();
    let witness = generate_witness().await;

    let mut stdin = SP1Stdin::new();
    stdin.write(&witness.oracle_data);
    let sp1_witness = Sp1Witness { stdin };

    let elf = open_zk_host::include_range_ethereum_elf!();
    let program = Sp1Program::new("range-ethereum", elf.to_vec());

    println!("Executing range-ethereum ELF in SP1 mock mode...");
    let start = Instant::now();
    let backend = Sp1ProverBackend::new().await;
    let result = backend
        .prove(&program, &sp1_witness, ProvingMode::Execute)
        .await
        .expect("SP1 execute failed");
    println!(
        "Execution completed in {:.2}s",
        start.elapsed().as_secs_f64()
    );
    println!("  cycle_count: {:?}", result.cycle_count);
    println!("  public_values: {} bytes", result.public_values.len());

    verify_journal(&result).await;
    println!("E2E test PASSED — range-ethereum guest correctly derived L2 state transition (SP1)");
}

#[cfg(feature = "risc0")]
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn test_range_ethereum_risc0_e2e_devnet() {
    use open_zk_host::prover::{RiscZeroProverBackend, RiscZeroProgram, RiscZeroWitness};

    init_tracing();
    let witness = generate_witness().await;

    let rz_witness = RiscZeroWitness {
        oracle_data: witness.oracle_data,
    };

    let elf = open_zk_host::elf::risc0::GUEST_RANGE_ETHEREUM_ELF;
    let image_id = open_zk_host::elf::risc0::GUEST_RANGE_ETHEREUM_ID;
    let program = RiscZeroProgram::new("range-ethereum", image_id, elf.to_vec());

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

    verify_journal(&result).await;
    println!("E2E test PASSED — range-ethereum guest correctly derived L2 state transition (RISC Zero)");
}
