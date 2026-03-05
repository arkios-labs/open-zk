//! Guest ELF binary constants.
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

#[cfg(feature = "sp1")]
pub use open_zk_zkvm_sp1_host::include_range_ethereum_elf;

#[cfg(feature = "risc0")]
pub mod risc0 {
    #[cfg(feature = "rebuild-risc0-guest")]
    pub use open_zk_zkvm_risc0_host::elf::*;
}
