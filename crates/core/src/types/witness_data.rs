use alloy_primitives::B256;
use alloy_sol_types::{sol, SolValue};
use serde::{Deserialize, Serialize};

extern crate alloc;
use alloc::vec::Vec;

/// Boot information for a single OP Stack state transition proof.
///
/// Contains the L1/L2 anchors and claim that the guest program must verify.
/// Analogous to kona's `BootInfo` / OP Succinct's `BootInfoStruct`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootInfo {
    /// L1 block hash used as trust anchor for derivation.
    pub l1_head: B256,
    /// L2 output root before the proven block.
    pub l2_pre_root: B256,
    /// Claimed L2 output root after execution.
    pub l2_claim: B256,
    /// L2 block number to derive and execute up to.
    pub l2_block_number: u64,
    /// Hash of the rollup chain configuration.
    pub rollup_config_hash: B256,
}

// ABI-compatible Solidity struct for BootInfo.
sol! {
    struct BootInfoOutput {
        bytes32 l1Head;
        bytes32 l2PreRoot;
        bytes32 l2Claim;
        uint64 l2BlockNumber;
        bytes32 rollupConfigHash;
    }
}

impl BootInfo {
    /// ABI-encode the boot info (160 bytes = 5 slots).
    pub fn to_abi_bytes(&self) -> Vec<u8> {
        let output = BootInfoOutput {
            l1Head: self.l1_head,
            l2PreRoot: self.l2_pre_root,
            l2Claim: self.l2_claim,
            l2BlockNumber: self.l2_block_number,
            rollupConfigHash: self.rollup_config_hash,
        };
        output.abi_encode_params()
    }

    /// Decode boot info from ABI-encoded bytes.
    pub fn from_abi_bytes(data: &[u8]) -> Result<Self, alloy_sol_types::Error> {
        let output = BootInfoOutput::abi_decode_params(data)?;
        Ok(Self {
            l1_head: output.l1Head,
            l2_pre_root: output.l2PreRoot,
            l2_claim: output.l2Claim,
            l2_block_number: output.l2BlockNumber,
            rollup_config_hash: output.rollupConfigHash,
        })
    }
}

/// Input data for the aggregation guest program.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationInput {
    /// Number of range proofs to aggregate.
    pub num_proofs: u32,
    /// Program ID (verification key) of the range proof program.
    pub range_program_id: B256,
    /// ABI-encoded journals from each range proof.
    pub journals: Vec<Vec<u8>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_boot_info() -> BootInfo {
        BootInfo {
            l1_head: B256::repeat_byte(0x11),
            l2_pre_root: B256::repeat_byte(0x22),
            l2_claim: B256::repeat_byte(0x33),
            l2_block_number: 1000,
            rollup_config_hash: B256::repeat_byte(0x44),
        }
    }

    #[test]
    fn boot_info_abi_roundtrip() {
        let boot = sample_boot_info();
        let bytes = boot.to_abi_bytes();
        assert_eq!(bytes.len(), 160); // 5 × 32-byte ABI slots
        let decoded = BootInfo::from_abi_bytes(&bytes).unwrap();
        assert_eq!(boot, decoded);
    }

    #[test]
    fn boot_info_abi_zero_values() {
        let boot = BootInfo {
            l1_head: B256::ZERO,
            l2_pre_root: B256::ZERO,
            l2_claim: B256::ZERO,
            l2_block_number: 0,
            rollup_config_hash: B256::ZERO,
        };
        let bytes = boot.to_abi_bytes();
        assert_eq!(bytes.len(), 160);
        let decoded = BootInfo::from_abi_bytes(&bytes).unwrap();
        assert_eq!(boot, decoded);
    }

    #[test]
    fn aggregation_input_serde_roundtrip() {
        let input = AggregationInput {
            num_proofs: 3,
            range_program_id: B256::repeat_byte(0xFF),
            journals: vec![vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9]],
        };

        let json = serde_json::to_string(&input).unwrap();
        let decoded: AggregationInput = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.num_proofs, 3);
        assert_eq!(decoded.journals.len(), 3);
    }
}
