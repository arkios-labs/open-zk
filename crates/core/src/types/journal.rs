use alloy_primitives::B256;
use alloy_sol_types::sol;
use serde::{Deserialize, Serialize};

/// Unified journal output for OP Stack state transition proofs.
///
/// Both Kailua's `ProofJournal` and OP Succinct's `BootInfoStruct` produce
/// equivalent data. This struct unifies them so the on-chain gateway can
/// decode outputs regardless of which backend generated the proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateTransitionJournal {
    /// L1 block hash used as the trust anchor.
    pub l1_head: B256,
    /// L2 output root before the proven range.
    pub l2_pre_root: B256,
    /// L2 output root after the proven range.
    pub l2_post_root: B256,
    /// L2 block number at the end of the proven range.
    pub l2_block_number: u64,
    /// Hash of the rollup chain configuration.
    pub rollup_config_hash: B256,
    /// Identifier of the guest program that produced this journal.
    pub program_id: B256,
}

// ABI-compatible Solidity struct for on-chain decoding.
sol! {
    struct StateTransitionOutput {
        bytes32 l1Head;
        bytes32 l2PreRoot;
        bytes32 l2PostRoot;
        uint64 l2BlockNumber;
        bytes32 rollupConfigHash;
        bytes32 programId;
    }
}
