//! Witness adapter utilities for converting RawWitness to backend-specific formats.
//!
//! When a zkVM backend needs witness data in a particular format (e.g., SP1's
//! `SP1Stdin` or RISC Zero's `ExecutorEnv`), these adapters handle the conversion.
//!
//! The guest reads oracle_data (rkyv-serialized preimages including boot info)
//! via a single `io.read::<Vec<u8>>()` call.

use open_zk_core::traits::RawWitness;

/// Convert a RawWitness into an SP1 witness.
///
/// The oracle_data already contains all preimages (including boot info as
/// local preimage keys), so we write it as a single `Vec<u8>`.
#[cfg(feature = "sp1")]
pub fn raw_witness_to_sp1_witness(
    witness: &RawWitness,
) -> Result<crate::prover::Sp1Witness, String> {
    let mut stdin = sp1_sdk::SP1Stdin::new();
    stdin.write(&witness.oracle_data);

    Ok(crate::prover::Sp1Witness { stdin })
}

/// Convert a RawWitness into a RISC Zero witness.
///
/// The oracle_data already contains all preimages (including boot info as
/// local preimage keys), so we just pass it through as-is.
#[cfg(feature = "risczero")]
pub fn raw_witness_to_risczero_witness(
    witness: &RawWitness,
) -> Result<crate::prover::RiscZeroWitness, String> {
    Ok(crate::prover::RiscZeroWitness {
        oracle_data: witness.oracle_data.clone(),
    })
}

/// Encode a RawWitness into a length-prefixed byte buffer for transport/storage.
///
/// Wire format:
///   [boot_info_len: u32 LE][boot_info bytes]
///   [oracle_data_len: u32 LE][oracle_data bytes]
///   [blob_data_len: u32 LE][blob_data bytes]
pub fn raw_witness_to_bytes(witness: &RawWitness) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&(witness.boot_info.len() as u32).to_le_bytes());
    buf.extend_from_slice(&witness.boot_info);
    buf.extend_from_slice(&(witness.oracle_data.len() as u32).to_le_bytes());
    buf.extend_from_slice(&witness.oracle_data);
    buf.extend_from_slice(&(witness.blob_data.len() as u32).to_le_bytes());
    buf.extend_from_slice(&witness.blob_data);
    buf
}

/// Decode a RawWitness from a length-prefixed byte buffer.
///
/// Inverse of `raw_witness_to_bytes`.
pub fn bytes_to_raw_witness(data: &[u8]) -> Option<RawWitness> {
    let mut offset = 0;

    let read_chunk = |data: &[u8], offset: &mut usize| -> Option<Vec<u8>> {
        if *offset + 4 > data.len() {
            return None;
        }
        let len = u32::from_le_bytes(data[*offset..*offset + 4].try_into().ok()?) as usize;
        *offset += 4;
        if *offset + len > data.len() {
            return None;
        }
        let chunk = data[*offset..*offset + len].to_vec();
        *offset += len;
        Some(chunk)
    };

    let boot_info = read_chunk(data, &mut offset)?;
    let oracle_data = read_chunk(data, &mut offset)?;
    let blob_data = read_chunk(data, &mut offset)?;

    Some(RawWitness {
        boot_info,
        oracle_data,
        blob_data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_witness() -> RawWitness {
        RawWitness {
            boot_info: vec![],
            oracle_data: b"oracle-preimages".to_vec(),
            blob_data: b"blob-sidecar".to_vec(),
        }
    }

    #[test]
    fn bytes_roundtrip() {
        let witness = sample_witness();
        let buf = raw_witness_to_bytes(&witness);
        let decoded = bytes_to_raw_witness(&buf).unwrap();
        assert_eq!(decoded.boot_info, witness.boot_info);
        assert_eq!(decoded.oracle_data, witness.oracle_data);
        assert_eq!(decoded.blob_data, witness.blob_data);
    }

    #[test]
    fn empty_witness_roundtrip() {
        let witness = RawWitness {
            boot_info: vec![],
            oracle_data: vec![],
            blob_data: vec![],
        };
        let buf = raw_witness_to_bytes(&witness);
        let decoded = bytes_to_raw_witness(&buf).unwrap();
        assert!(decoded.boot_info.is_empty());
        assert!(decoded.oracle_data.is_empty());
        assert!(decoded.blob_data.is_empty());
    }

    #[test]
    fn truncated_data_returns_none() {
        assert!(bytes_to_raw_witness(&[]).is_none());
        assert!(bytes_to_raw_witness(&[5, 0, 0, 0]).is_none());
    }

    #[cfg(feature = "sp1")]
    #[test]
    fn sp1_witness_from_raw() {
        let witness = sample_witness();
        let sp1_witness = raw_witness_to_sp1_witness(&witness).unwrap();
        let _ = sp1_witness;
    }

    #[cfg(feature = "risczero")]
    #[test]
    fn risczero_witness_from_raw() {
        let witness = sample_witness();
        let rz_witness = raw_witness_to_risczero_witness(&witness).unwrap();
        assert_eq!(rz_witness.oracle_data, b"oracle-preimages");
    }
}
