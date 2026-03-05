//! ABI encoding tests for contract interfaces.

use alloy_sol_types::{SolCall, SolInterface};
use open_zk_contracts::abi::IOpenZkL2OutputOracle;

/// submitSp1Proof calldata can be encoded and decoded.
#[test]
fn submit_sp1_proof_calldata_roundtrip() {
    let journal_bytes = vec![0u8; 192];
    let call = IOpenZkL2OutputOracle::submitSp1ProofCall {
        publicValues: journal_bytes.clone().into(),
        proofBytes: vec![0xDE, 0xAD].into(),
    };

    let encoded = call.abi_encode();
    assert!(!encoded.is_empty());

    let decoded =
        IOpenZkL2OutputOracle::IOpenZkL2OutputOracleCalls::abi_decode(&encoded).unwrap();
    match decoded {
        IOpenZkL2OutputOracle::IOpenZkL2OutputOracleCalls::submitSp1Proof(inner) => {
            assert_eq!(inner.publicValues.len(), 192);
            assert_eq!(inner.proofBytes.as_ref(), &[0xDE, 0xAD]);
        }
        _ => panic!("expected submitSp1Proof variant"),
    }
}

/// submitRisc0Proof calldata can be encoded and decoded.
#[test]
fn submit_risc0_proof_calldata_roundtrip() {
    let journal_bytes = vec![0u8; 192];
    let call = IOpenZkL2OutputOracle::submitRisc0ProofCall {
        journalBytes: journal_bytes.clone().into(),
        seal: vec![0xCA, 0xFE].into(),
    };

    let encoded = call.abi_encode();
    let decoded =
        IOpenZkL2OutputOracle::IOpenZkL2OutputOracleCalls::abi_decode(&encoded).unwrap();
    match decoded {
        IOpenZkL2OutputOracle::IOpenZkL2OutputOracleCalls::submitRisc0Proof(inner) => {
            assert_eq!(inner.journalBytes.len(), 192);
            assert_eq!(inner.seal.as_ref(), &[0xCA, 0xFE]);
        }
        _ => panic!("expected submitRisc0Proof variant"),
    }
}

/// View function selectors are distinct and encode correctly.
#[test]
fn view_functions_encode() {
    let call1 = IOpenZkL2OutputOracle::latestOutputRootCall {};
    let encoded1 = call1.abi_encode();
    assert_eq!(encoded1.len(), 4);

    let call2 = IOpenZkL2OutputOracle::latestBlockNumberCall {};
    let encoded2 = call2.abi_encode();
    assert_eq!(encoded2.len(), 4);

    assert_ne!(encoded1, encoded2);

    let call3 = IOpenZkL2OutputOracle::isBlockProvenCall { blockNumber: 100 };
    let encoded3 = call3.abi_encode();
    assert_eq!(encoded3.len(), 36);
}
