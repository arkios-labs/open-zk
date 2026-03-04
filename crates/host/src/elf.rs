//! Guest ELF binary constants.
//!
//! After building guest programs with the appropriate zkVM toolchain,
//! the compiled ELF binaries are referenced here for use by the prover.
//!
//! # Build Instructions
//!
//! ## SP1 (recommended)
//!
//! ```bash
//! # Install SP1 toolchain
//! curl -L https://sp1.succinct.xyz | bash
//! sp1up
//!
//! # Build range proof guest
//! cd guests/range
//! cargo prove build --features sp1
//!
//! # Build aggregation guest
//! cd guests/aggregation
//! cargo prove build --features sp1
//! ```
//!
//! ## RISC Zero
//!
//! ```bash
//! # Install RISC Zero toolchain
//! curl -L https://risczero.com/install | bash
//! rzup install
//!
//! # Build range proof guest
//! cd guests/range
//! cargo risczero build --features risczero
//!
//! # Build aggregation guest
//! cd guests/aggregation
//! cargo risczero build --features risczero
//! ```
//!
//! # Automated Build
//!
//! When building with SP1, you can use `sp1-build` in the host crate's `build.rs`
//! to compile guests automatically:
//!
//! ```rust,ignore
//! // build.rs
//! use sp1_build::build_program;
//! fn main() {
//!     build_program("../../guests/range");
//!     build_program("../../guests/aggregation");
//! }
//! ```

/// Path to the range proof guest ELF binary (SP1).
///
/// Set to the output of `cargo prove build` in `guests/range/`.
/// Default: `guests/range/elf/riscv32im-succinct-zkvm-elf`
pub const RANGE_ELF_PATH: &str = "guests/range/elf/riscv32im-succinct-zkvm-elf";

/// Path to the aggregation guest ELF binary (SP1).
///
/// Set to the output of `cargo prove build` in `guests/aggregation/`.
/// Default: `guests/aggregation/elf/riscv32im-succinct-zkvm-elf`
pub const AGGREGATION_ELF_PATH: &str =
    "guests/aggregation/elf/riscv32im-succinct-zkvm-elf";

/// Include the range proof ELF at compile time (requires prior build).
///
/// Use this when the ELF has been pre-built and placed at the expected path.
/// Returns `None` if the ELF file doesn't exist at compile time.
///
/// For production use, prefer `sp1-build` in `build.rs` which builds and
/// includes the ELF automatically.
#[macro_export]
macro_rules! include_range_elf {
    () => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../guests/range/elf/riscv32im-succinct-zkvm-elf"
        ))
    };
}

/// Include the aggregation ELF at compile time (requires prior build).
#[macro_export]
macro_rules! include_aggregation_elf {
    () => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../guests/aggregation/elf/riscv32im-succinct-zkvm-elf"
        ))
    };
}
