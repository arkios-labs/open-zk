//! Guest ELF binary constants.
//!
//! Re-exports from the zkvm backend crates.

#[cfg(feature = "sp1")]
pub use open_zk_sp1::include_range_ethereum_elf;

#[cfg(feature = "risc0")]
pub mod risc0 {
    pub use open_zk_risc0::{GUEST_RANGE_ETHEREUM_RISC0_ELF, GUEST_RANGE_ETHEREUM_RISC0_ID};
}
