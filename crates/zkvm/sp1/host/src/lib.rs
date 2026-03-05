mod prover;
mod witness;

pub use prover::{Sp1Program, Sp1ProverBackend, Sp1ProverError};
pub use witness::{Sp1Witness, raw_witness_to_sp1_witness};

/// Path to the SP1 range-ethereum ELF relative to the workspace root.
pub const RANGE_ETHEREUM_ELF_PATH: &str =
    "guests/range-ethereum/sp1/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/guest-range-ethereum-sp1";

/// Load the SP1 range-ethereum ELF at runtime from the workspace root.
///
/// Searches upward from `CARGO_MANIFEST_DIR` for the workspace root (containing `Cargo.lock`),
/// then reads the ELF from the expected path.
pub fn load_range_ethereum_elf() -> Vec<u8> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR not set");
    let mut dir = std::path::PathBuf::from(manifest_dir);
    // Walk up until we find the workspace root (has Cargo.lock)
    while !dir.join("Cargo.lock").exists() {
        if !dir.pop() {
            panic!("could not find workspace root from CARGO_MANIFEST_DIR");
        }
    }
    let elf_path = dir.join(RANGE_ETHEREUM_ELF_PATH);
    std::fs::read(&elf_path).unwrap_or_else(|e| {
        panic!(
            "failed to read SP1 ELF at {}: {e}\nBuild it first: cd guests/range-ethereum/sp1 && cargo prove build --features sp1",
            elf_path.display()
        )
    })
}
