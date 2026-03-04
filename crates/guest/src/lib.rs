#![cfg_attr(not(test), no_std)]

#[cfg(feature = "sp1")]
mod sp1_io;

#[cfg(feature = "risczero")]
mod risczero_io;

#[cfg(feature = "sp1")]
pub use sp1_io::Sp1Io;

#[cfg(feature = "risczero")]
pub use risczero_io::RiscZeroIo;

/// Returns the I/O handle for the current zkVM backend.
///
/// This is the primary entry point for guest programs:
/// ```ignore
/// let io = open_zk_guest::io();
/// let boot_info: BootInfo = io.read();
/// // ... run derivation and execution ...
/// io.commit(&journal);
/// ```
#[cfg(feature = "sp1")]
pub fn io() -> Sp1Io {
    Sp1Io
}

#[cfg(feature = "risczero")]
pub fn io() -> RiscZeroIo {
    RiscZeroIo
}
