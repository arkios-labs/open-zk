//! Solidity ABI bindings and on-chain interaction traits for open-zk.

pub mod abi;
pub mod client;

pub use abi::*;
pub use client::{MockProofSubmitter, ProofSubmitter};
