mod prover;
mod witness;

pub use prover::{Sp1Program, Sp1ProverBackend, Sp1ProverError};
pub use witness::{Sp1Witness, raw_witness_to_sp1_witness};

/// Include the Ethereum DA range proof ELF (SP1) at compile time.
#[macro_export]
macro_rules! include_range_ethereum_elf {
    () => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../guests/range-ethereum/sp1/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/guest-range-ethereum-sp1"
        ))
    };
}
