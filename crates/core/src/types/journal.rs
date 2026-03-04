use alloy_primitives::B256;
use alloy_sol_types::{sol, SolValue};
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

impl StateTransitionJournal {
    /// ABI-encode the journal for on-chain submission.
    ///
    /// Produces 192 bytes (6 ABI-encoded 32-byte slots).
    pub fn to_abi_bytes(&self) -> Vec<u8> {
        let output = StateTransitionOutput {
            l1Head: self.l1_head,
            l2PreRoot: self.l2_pre_root,
            l2PostRoot: self.l2_post_root,
            l2BlockNumber: self.l2_block_number,
            rollupConfigHash: self.rollup_config_hash,
            programId: self.program_id,
        };
        output.abi_encode_params()
    }

    /// Decode a journal from ABI-encoded bytes.
    pub fn from_abi_bytes(data: &[u8]) -> Result<Self, alloy_sol_types::Error> {
        let output = StateTransitionOutput::abi_decode_params(data)?;
        Ok(Self {
            l1_head: output.l1Head,
            l2_pre_root: output.l2PreRoot,
            l2_post_root: output.l2PostRoot,
            l2_block_number: output.l2BlockNumber,
            rollup_config_hash: output.rollupConfigHash,
            program_id: output.programId,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_journal() -> StateTransitionJournal {
        StateTransitionJournal {
            l1_head: B256::repeat_byte(0x01),
            l2_pre_root: B256::repeat_byte(0x02),
            l2_post_root: B256::repeat_byte(0x03),
            l2_block_number: 42,
            rollup_config_hash: B256::repeat_byte(0x04),
            program_id: B256::repeat_byte(0x05),
        }
    }

    #[test]
    fn abi_roundtrip() {
        let journal = sample_journal();
        let bytes = journal.to_abi_bytes();
        assert_eq!(bytes.len(), 192); // 6 × 32-byte ABI slots
        let decoded = StateTransitionJournal::from_abi_bytes(&bytes).unwrap();
        assert_eq!(journal, decoded);
    }

    #[test]
    fn abi_roundtrip_zero_values() {
        let journal = StateTransitionJournal {
            l1_head: B256::ZERO,
            l2_pre_root: B256::ZERO,
            l2_post_root: B256::ZERO,
            l2_block_number: 0,
            rollup_config_hash: B256::ZERO,
            program_id: B256::ZERO,
        };
        let bytes = journal.to_abi_bytes();
        assert_eq!(bytes.len(), 192);
        let decoded = StateTransitionJournal::from_abi_bytes(&bytes).unwrap();
        assert_eq!(journal, decoded);
    }
}
