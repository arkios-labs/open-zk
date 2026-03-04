//! Guest program execution tests for SP1 and RISC Zero backends.
//!
//! These tests verify the full host→guest→journal pipeline:
//! 1. Create a BootInfo with test values
//! 2. Serialize witness data into backend-specific format
//! 3. Execute the guest ELF in mock/dev mode
//! 4. Decode the journal from public values
//! 5. Verify the journal matches expected values
//!
//! Prerequisites:
//!   - SP1: `sp1up` installed, `cd guests/range && cargo prove build --features sp1`
//!   - RISC Zero: `rzup install`, `cd guests/range && cargo +risc0 build ...`
//!
//! Run:
//!   SP1_PROVER=mock cargo test -p open-zk-host --features sp1 --test execution_test -- --ignored --nocapture
//!   RISC0_DEV_MODE=1 cargo test -p open-zk-host --features risczero --test execution_test -- --ignored --nocapture

use alloy_primitives::B256;
use open_zk_core::types::{BootInfo, StateTransitionJournal};

fn test_boot_info() -> BootInfo {
    BootInfo {
        l1_head: B256::repeat_byte(0x11),
        l2_pre_root: B256::repeat_byte(0x22),
        l2_claim: B256::repeat_byte(0x33),
        l2_block_number: 42,
        rollup_config_hash: B256::repeat_byte(0x44),
    }
}

/// Test SP1 execute mode with the range guest program.
///
/// Uses `SP1_PROVER=mock` for fast execution without real ZK proof generation.
/// The guest reads BootInfo + witness bytes, produces a StateTransitionJournal.
#[cfg(feature = "sp1")]
#[tokio::test]
#[ignore]
async fn test_sp1_execute_range_guest() {
    use open_zk_core::traits::ProverBackend;
    use open_zk_core::types::ProvingMode;
    use open_zk_host::prover::{Sp1ProverBackend, Sp1Program, Sp1Witness};
    use sp1_sdk::SP1Stdin;

    let boot_info = test_boot_info();
    let witness_data: Vec<u8> = vec![]; // Empty oracle data — skeleton guest ignores it

    // Create SP1 stdin with typed writes matching guest's io.read::<T>() calls
    let mut stdin = SP1Stdin::new();
    stdin.write(&boot_info);
    stdin.write(&witness_data);

    let witness = Sp1Witness { stdin };

    // Load the pre-built ELF
    let elf = open_zk_host::include_range_elf!();
    let program = Sp1Program::new("range", elf.to_vec());

    // Execute (no proof generation)
    let backend = Sp1ProverBackend::new().await;
    let result = backend
        .prove(&program, &witness, ProvingMode::Execute)
        .await
        .expect("SP1 execute failed");

    println!("SP1 execute completed");
    println!("  cycle_count: {:?}", result.cycle_count);
    println!("  public_values: {} bytes", result.public_values.len());

    // Decode journal from public values
    assert!(
        !result.public_values.is_empty(),
        "public_values should not be empty"
    );

    let journal = StateTransitionJournal::from_abi_bytes(&result.public_values)
        .expect("failed to decode journal from public values");

    // Verify journal matches boot_info
    assert_eq!(journal.l1_head, boot_info.l1_head);
    assert_eq!(journal.l2_pre_root, boot_info.l2_pre_root);
    assert_eq!(journal.l2_post_root, boot_info.l2_claim); // Skeleton guest trusts the claim
    assert_eq!(journal.l2_block_number, boot_info.l2_block_number);
    assert_eq!(journal.rollup_config_hash, boot_info.rollup_config_hash);

    println!("Journal verified:");
    println!("  l1_head: {}", journal.l1_head);
    println!("  l2_pre_root: {}", journal.l2_pre_root);
    println!("  l2_post_root: {}", journal.l2_post_root);
    println!("  l2_block_number: {}", journal.l2_block_number);
}

/// Test the full adapter pipeline: RawWitness → SP1 witness → execute → journal.
///
/// Verifies that the `raw_witness_to_sp1_witness` adapter correctly bridges
/// the RawWitness (ABI-encoded BootInfo + serialized preimages) to SP1's
/// typed stdin format.
#[cfg(feature = "sp1")]
#[tokio::test]
#[ignore]
async fn test_sp1_adapter_pipeline() {
    use open_zk_core::traits::{ProverBackend, RawWitness};
    use open_zk_core::types::ProvingMode;
    use open_zk_host::prover::{Sp1ProverBackend, Sp1Program};
    use open_zk_host::witness::raw_witness_to_sp1_witness;

    let boot_info = test_boot_info();

    // Create a RawWitness matching what RpcWitnessProvider produces
    let raw = RawWitness {
        boot_info: boot_info.to_abi_bytes(),
        oracle_data: vec![], // Empty for skeleton guest
        blob_data: vec![],
    };

    // Convert through the adapter
    let witness = raw_witness_to_sp1_witness(&raw).expect("adapter conversion failed");

    let elf = open_zk_host::include_range_elf!();
    let program = Sp1Program::new("range", elf.to_vec());

    let backend = Sp1ProverBackend::new().await;
    let result = backend
        .prove(&program, &witness, ProvingMode::Execute)
        .await
        .expect("SP1 execute via adapter failed");

    let journal = StateTransitionJournal::from_abi_bytes(&result.public_values)
        .expect("failed to decode journal");

    assert_eq!(journal.l1_head, boot_info.l1_head);
    assert_eq!(journal.l2_post_root, boot_info.l2_claim);
    println!("SP1 adapter pipeline test passed");
}

/// Test RISC Zero execute mode with the range guest program.
///
/// Uses `RISC0_DEV_MODE=1` for fast execution without real ZK proof generation.
#[cfg(feature = "risczero")]
#[tokio::test]
#[ignore]
async fn test_risczero_execute_range_guest() {
    use open_zk_core::traits::ProverBackend;
    use open_zk_core::types::ProvingMode;
    use open_zk_host::prover::{RiscZeroProverBackend, RiscZeroProgram, RiscZeroWitness};

    let boot_info = test_boot_info();

    let witness = RiscZeroWitness {
        boot_info: boot_info.clone(),
        witness_data: vec![], // Empty oracle data — skeleton guest ignores it
    };

    // Load the pre-built RISC Zero ELF + compute image ID
    // TODO: Update path once RISC Zero build is working
    let elf_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../guests/range/target/riscv32im-risc0-zkvm-elf/release/guest-range"
    );
    let elf = std::fs::read(elf_path).expect("RISC Zero ELF not found — build with cargo risczero build first");
    let image_id = risc0_zkvm::compute_image_id(&elf).expect("failed to compute image ID");
    let image_id_words: [u32; 8] = image_id.into();

    let program = RiscZeroProgram::new("range", image_id_words, elf);

    let backend = RiscZeroProverBackend::new();
    let result = backend
        .prove(&program, &witness, ProvingMode::Execute)
        .await
        .expect("RISC Zero execute failed");

    println!("RISC Zero execute completed");
    println!("  cycle_count: {:?}", result.cycle_count);
    println!("  public_values: {} bytes", result.public_values.len());

    assert!(
        !result.public_values.is_empty(),
        "public_values should not be empty"
    );

    let journal = StateTransitionJournal::from_abi_bytes(&result.public_values)
        .expect("failed to decode journal from public values");

    assert_eq!(journal.l1_head, boot_info.l1_head);
    assert_eq!(journal.l2_pre_root, boot_info.l2_pre_root);
    assert_eq!(journal.l2_post_root, boot_info.l2_claim);
    assert_eq!(journal.l2_block_number, boot_info.l2_block_number);

    println!("Journal verified (RISC Zero)");
}
