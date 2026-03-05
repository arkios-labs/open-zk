//! Guest ELF binary constants.
//!
//! After building guest programs with the appropriate zkVM toolchain,
//! the compiled ELF binaries are referenced here for use by the prover.
//!
//! # Build Instructions
//!
//! ## SP1
//!
//! ```bash
//! cd guests/range-ethereum/sp1
//! cargo prove build --features sp1
//! ```
//!
//! ## RISC Zero
//!
//! ```bash
//! cargo build -p open-zk-build-risc0 --features rebuild-guest,debug-guest-build
//! ```

/// Include the Ethereum DA range proof ELF (SP1) at compile time.
#[macro_export]
macro_rules! include_range_ethereum_elf {
    () => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../guests/range-ethereum/sp1/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/guest-range-ethereum-sp1"
        ))
    };
}

/// Re-exports from the risc0 build crate.
#[cfg(feature = "risc0")]
pub mod risc0 {
    pub use open_zk_build_risc0::GUEST_RANGE_ETHEREUM_RISC0_ELF;
    pub use open_zk_build_risc0::GUEST_RANGE_ETHEREUM_RISC0_ID;
}
