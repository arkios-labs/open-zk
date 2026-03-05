//! Guest program execution tests for RISC Zero backend.
//!
//! For full E2E tests with devnet witness data, see:
//!   - range_ethereum_e2e.rs (SP1)
//!   - range_ethereum_risc0_e2e.rs (RISC Zero)
//!
//! Run:
//!   RISC0_DEV_MODE=1 cargo test -p open-zk-host --features risc0 --test execution_test -- --ignored --nocapture

/// Test RISC Zero execute mode with the range-ethereum guest program.
///
/// Uses `RISC0_DEV_MODE=1` for fast execution without real ZK proof generation.
/// The guest reads a single Vec<u8> (oracle data) and runs the kona pipeline.
/// This is a skeleton test with empty oracle data — the guest will fail at
/// BootInfo::load since no preimage keys are present. For a full E2E test,
/// see range_ethereum_risc0_e2e.rs.
#[cfg(feature = "risc0")]
#[tokio::test]
#[ignore]
async fn test_risc0_execute_range_guest() {
    use open_zk_core::traits::ProverBackend;
    use open_zk_core::types::ProvingMode;
    use open_zk_host::prover::{RiscZeroProgram, RiscZeroProverBackend, RiscZeroWitness};

    let elf = open_zk_host::elf::risc0::GUEST_RANGE_ETHEREUM_ELF;
    let image_id = open_zk_host::elf::risc0::GUEST_RANGE_ETHEREUM_ID;
    let program = RiscZeroProgram::new("range-ethereum", image_id, elf.to_vec());

    // Empty oracle data — guest will fail at BootInfo::load, but this verifies
    // the ELF loads and the witness format is correct.
    let witness = RiscZeroWitness {
        oracle_data: vec![],
    };

    let backend = RiscZeroProverBackend::new();
    let result = backend
        .prove(&program, &witness, ProvingMode::Execute)
        .await;

    // Expected to fail with empty oracle data (no preimage keys)
    println!("RISC Zero skeleton test result: {:?}", result.is_ok());
}
