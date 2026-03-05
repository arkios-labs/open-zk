//! RISC Zero guest ELF binaries and image IDs.
//!
//! When built with `rebuild-guest` feature, guest ELFs are compiled from source
//! using `risc0-build`. Otherwise, pre-built constants are used.

// When rebuild-guest is enabled, include the auto-generated methods.rs
#[cfg(feature = "rebuild-guest")]
include!(concat!(env!("OUT_DIR"), "/methods.rs"));

// When rebuild-guest is NOT enabled, use pre-built constants.
#[cfg(not(feature = "rebuild-guest"))]
mod prebuilt {
    /// Placeholder ELF — rebuild with `--features rebuild-guest` to generate.
    pub const GUEST_RANGE_ETHEREUM_RISC0_ELF: &[u8] = &[];
    /// Placeholder image ID — rebuild with `--features rebuild-guest` to generate.
    pub const GUEST_RANGE_ETHEREUM_RISC0_ID: [u32; 8] = [0u32; 8];
}

#[cfg(not(feature = "rebuild-guest"))]
pub use prebuilt::*;
