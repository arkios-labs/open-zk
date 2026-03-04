//! ABI encoding tests for contract interfaces.

use alloy_primitives::B256;
use alloy_sol_types::{SolCall, SolInterface};
use open_zk_contracts::abi::{IOpenZkDisputeGame, IOpenZkL2OutputOracle};
use open_zk_core::types::StateTransitionJournal;

/// submitProof calldata can be encoded and decoded.
#[test]
fn submit_proof_calldata_roundtrip() {
    let call = IOpenZkL2OutputOracle::submitProofCall {
        l1Head: B256::repeat_byte(0x01),
        l2PreRoot: B256::repeat_byte(0x02),
        l2PostRoot: B256::repeat_byte(0x03),
        l2BlockNumber: 42,
        rollupConfigHash: B256::repeat_byte(0x04),
        programId: B256::repeat_byte(0x05),
        proof: vec![0xDE, 0xAD].into(),
    };

    let encoded = call.abi_encode();
    assert!(!encoded.is_empty());

    // Decode using SolInterface which handles the 4-byte selector
    let decoded = IOpenZkL2OutputOracle::IOpenZkL2OutputOracleCalls::abi_decode(&encoded).unwrap();
    match decoded {
        IOpenZkL2OutputOracle::IOpenZkL2OutputOracleCalls::submitProof(inner) => {
            assert_eq!(inner.l1Head, B256::repeat_byte(0x01));
            assert_eq!(inner.l2BlockNumber, 42);
            assert_eq!(inner.proof.as_ref(), &[0xDE, 0xAD]);
        }
        _ => panic!("expected submitProof variant"),
    }
}

/// Journal ABI encoding is compatible with submitProof parameters.
#[test]
fn journal_abi_compatible_with_submit_proof() {
    let journal = StateTransitionJournal {
        l1_head: B256::repeat_byte(0x11),
        l2_pre_root: B256::repeat_byte(0x22),
        l2_post_root: B256::repeat_byte(0x33),
        l2_block_number: 1000,
        rollup_config_hash: B256::repeat_byte(0x44),
        program_id: B256::repeat_byte(0x55),
    };

    let call = IOpenZkL2OutputOracle::submitProofCall {
        l1Head: journal.l1_head,
        l2PreRoot: journal.l2_pre_root,
        l2PostRoot: journal.l2_post_root,
        l2BlockNumber: journal.l2_block_number,
        rollupConfigHash: journal.rollup_config_hash,
        programId: journal.program_id,
        proof: vec![].into(),
    };

    let encoded = call.abi_encode();
    let decoded = IOpenZkL2OutputOracle::IOpenZkL2OutputOracleCalls::abi_decode(&encoded).unwrap();
    match decoded {
        IOpenZkL2OutputOracle::IOpenZkL2OutputOracleCalls::submitProof(inner) => {
            assert_eq!(inner.l1Head, journal.l1_head);
            assert_eq!(inner.l2PreRoot, journal.l2_pre_root);
            assert_eq!(inner.l2PostRoot, journal.l2_post_root);
            assert_eq!(inner.l2BlockNumber, journal.l2_block_number);
            assert_eq!(inner.rollupConfigHash, journal.rollup_config_hash);
            assert_eq!(inner.programId, journal.program_id);
        }
        _ => panic!("expected submitProof variant"),
    }
}

/// Dispute game resolve calldata encoding.
#[test]
fn resolve_dispute_calldata_roundtrip() {
    let call = IOpenZkDisputeGame::resolveCall {
        l1Head: B256::repeat_byte(0xAA),
        l2PreRoot: B256::repeat_byte(0xBB),
        l2PostRoot: B256::repeat_byte(0xCC),
        l2BlockNumber: 9999,
        rollupConfigHash: B256::repeat_byte(0xDD),
        programId: B256::repeat_byte(0xEE),
        proof: vec![1, 2, 3, 4].into(),
    };

    let encoded = call.abi_encode();
    let decoded = IOpenZkDisputeGame::IOpenZkDisputeGameCalls::abi_decode(&encoded).unwrap();
    match decoded {
        IOpenZkDisputeGame::IOpenZkDisputeGameCalls::resolve(inner) => {
            assert_eq!(inner.l1Head, B256::repeat_byte(0xAA));
            assert_eq!(inner.l2BlockNumber, 9999);
            assert_eq!(inner.proof.as_ref(), &[1, 2, 3, 4]);
        }
        _ => panic!("expected resolve variant"),
    }
}

/// Challenge calldata encoding.
#[test]
fn challenge_calldata_encoding() {
    let call = IOpenZkDisputeGame::challengeCall { blockNumber: 500 };

    let encoded = call.abi_encode();
    let decoded = IOpenZkDisputeGame::IOpenZkDisputeGameCalls::abi_decode(&encoded).unwrap();
    match decoded {
        IOpenZkDisputeGame::IOpenZkDisputeGameCalls::challenge(inner) => {
            assert_eq!(inner.blockNumber, 500);
        }
        _ => panic!("expected challenge variant"),
    }
}

/// View function selectors are distinct and encode correctly.
#[test]
fn view_functions_encode() {
    let call1 = IOpenZkL2OutputOracle::latestOutputRootCall {};
    let encoded1 = call1.abi_encode();
    assert_eq!(encoded1.len(), 4); // Just the selector, no args

    let call2 = IOpenZkL2OutputOracle::latestBlockNumberCall {};
    let encoded2 = call2.abi_encode();
    assert_eq!(encoded2.len(), 4);

    // Selectors must be different
    assert_ne!(encoded1, encoded2);

    let call3 = IOpenZkL2OutputOracle::isBlockProvenCall { blockNumber: 100 };
    let encoded3 = call3.abi_encode();
    // 4 byte selector + 32 byte uint64 param
    assert_eq!(encoded3.len(), 36);
}
