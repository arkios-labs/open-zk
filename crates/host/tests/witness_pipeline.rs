//! Integration tests for the witness pipeline.
//!
//! Tests MockWitnessProvider roundtrip and witness adapter conversions.

use alloy_primitives::B256;
use open_zk_core::traits::{RawWitness, WitnessProvider};
use open_zk_core::types::{BootInfo, ProofRequest, ProvingMode};
use open_zk_host::witness::{bytes_to_raw_witness, MockWitnessProvider};

/// MockWitnessProvider generates deterministic witness data.
#[tokio::test]
async fn mock_witness_roundtrip() {
    let provider = MockWitnessProvider;
    let request = ProofRequest {
        l1_head: B256::repeat_byte(0xAA),
        l2_start_block: 100,
        l2_end_block: 200,
        l2_start_output_root: B256::repeat_byte(0xBB),
        mode: ProvingMode::Groth16,
    };

    let witness = provider.generate_witness(&request).await.unwrap();

    // Verify non-empty
    assert!(!witness.boot_info.is_empty());
    assert!(!witness.oracle_data.is_empty());
    assert!(!witness.blob_data.is_empty());

    // Verify deterministic
    let witness2 = provider.generate_witness(&request).await.unwrap();
    assert_eq!(witness.boot_info, witness2.boot_info);
    assert_eq!(witness.oracle_data, witness2.oracle_data);
    assert_eq!(witness.blob_data, witness2.blob_data);
}

/// Different requests produce different witnesses.
#[tokio::test]
async fn different_requests_different_witnesses() {
    let provider = MockWitnessProvider;

    let req1 = ProofRequest {
        l1_head: B256::ZERO,
        l2_start_block: 100,
        l2_end_block: 200,
        l2_start_output_root: B256::ZERO,
        mode: ProvingMode::Groth16,
    };

    let req2 = ProofRequest {
        l1_head: B256::ZERO,
        l2_start_block: 300,
        l2_end_block: 400,
        l2_start_output_root: B256::ZERO,
        mode: ProvingMode::Groth16,
    };

    let w1 = provider.generate_witness(&req1).await.unwrap();
    let w2 = provider.generate_witness(&req2).await.unwrap();

    assert_ne!(w1.boot_info, w2.boot_info);
    assert_ne!(w1.oracle_data, w2.oracle_data);
}

/// Witness adapter: serialize → deserialize roundtrip.
#[test]
fn witness_adapter_roundtrip() {
    let witness = RawWitness {
        boot_info: b"test-boot-info".to_vec(),
        oracle_data: b"test-oracle-data".to_vec(),
        blob_data: b"test-blob-data".to_vec(),
    };

    // Manually encode (same format as sp1/risczero adapters)
    let mut buf = Vec::new();
    for field in [&witness.boot_info, &witness.oracle_data, &witness.blob_data] {
        buf.extend_from_slice(&(field.len() as u32).to_le_bytes());
        buf.extend_from_slice(field);
    }

    let decoded = bytes_to_raw_witness(&buf).unwrap();
    assert_eq!(decoded.boot_info, witness.boot_info);
    assert_eq!(decoded.oracle_data, witness.oracle_data);
    assert_eq!(decoded.blob_data, witness.blob_data);
}

/// BootInfo can be serialized into witness boot_info field.
#[test]
fn boot_info_as_witness_data() {
    let boot = BootInfo {
        l1_head: B256::repeat_byte(0x11),
        l2_pre_root: B256::repeat_byte(0x22),
        l2_claim: B256::repeat_byte(0x33),
        l2_block_number: 12345,
        rollup_config_hash: B256::repeat_byte(0x44),
    };

    let abi_bytes = boot.to_abi_bytes();
    assert_eq!(abi_bytes.len(), 160);

    let decoded = BootInfo::from_abi_bytes(&abi_bytes).unwrap();
    assert_eq!(boot, decoded);
}
