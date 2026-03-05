//! Solidity ABI bindings and on-chain interaction traits for open-zk.

pub mod abi;
pub mod client;

#[cfg(feature = "rpc")]
pub mod rpc_submitter;

pub use abi::*;
pub use client::{MockProofSubmitter, ProofSubmitter};

#[cfg(feature = "rpc")]
pub use rpc_submitter::RpcProofSubmitter;
