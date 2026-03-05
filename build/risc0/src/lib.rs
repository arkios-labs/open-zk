//! RISC Zero guest ELF binaries and image IDs for open-zk.
//!
//! When built with `rebuild-guest` feature, guest ELFs are compiled from source
//! using `risc0-build`. Otherwise, pre-built constants are used.
//!
//! # Build from source (requires rzup)
//!
//! ```bash
//! # Debug (local cross-compile, fast):
//! cargo build -p open-zk-build-risc0 --features rebuild-guest,debug-guest-build
//!
//! # Release (Docker, reproducible):
//! cargo build -p open-zk-build-risc0 --features rebuild-guest --release
//! ```
//!
//! # Generated constants
//!
//! - `GUEST_RANGE_ETHEREUM_ELF: &[u8]` — compiled guest ELF binary
//! - `GUEST_RANGE_ETHEREUM_ID: [u32; 8]` — RISC Zero image ID

// When rebuild-guest is enabled, include the auto-generated methods.rs
#[cfg(feature = "rebuild-guest")]
include!(concat!(env!("OUT_DIR"), "/methods.rs"));

// When rebuild-guest is NOT enabled, use pre-built constants.
// These are updated by running the rebuild and copying the output.
#[cfg(not(feature = "rebuild-guest"))]
mod prebuilt {
    /// Placeholder ELF — rebuild with `--features rebuild-guest` to generate.
    pub const GUEST_RANGE_ETHEREUM_ELF: &[u8] = &[];
    /// Placeholder image ID — rebuild with `--features rebuild-guest` to generate.
    pub const GUEST_RANGE_ETHEREUM_ID: [u32; 8] = [0u32; 8];
}

#[cfg(not(feature = "rebuild-guest"))]
pub use prebuilt::*;
