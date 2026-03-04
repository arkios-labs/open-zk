//! Witness adapter utilities for converting RawWitness to backend-specific formats.
//!
//! When a zkVM backend needs witness data in a particular format (e.g., SP1's
//! `SP1Stdin` or RISC Zero's `ExecutorEnv`), these adapters handle the conversion.

use open_zk_core::traits::RawWitness;

/// Convert a RawWitness into SP1-compatible stdin data.
///
/// Returns the serialized bytes that can be fed into `SP1Stdin::write_slice`.
#[cfg(feature = "sp1")]
pub fn raw_witness_to_sp1_bytes(witness: &RawWitness) -> Vec<u8> {
    // SP1 stdin expects sequential writes:
    // 1. boot_info length + data
    // 2. oracle_data length + data
    // 3. blob_data length + data
    let mut buf = Vec::new();
    buf.extend_from_slice(&(witness.boot_info.len() as u32).to_le_bytes());
    buf.extend_from_slice(&witness.boot_info);
    buf.extend_from_slice(&(witness.oracle_data.len() as u32).to_le_bytes());
    buf.extend_from_slice(&witness.oracle_data);
    buf.extend_from_slice(&(witness.blob_data.len() as u32).to_le_bytes());
    buf.extend_from_slice(&witness.blob_data);
    buf
}

/// Convert a RawWitness into RISC Zero-compatible input data.
///
/// Returns the serialized bytes that can be fed into `ExecutorEnv::write_slice`.
#[cfg(feature = "risczero")]
pub fn raw_witness_to_risczero_bytes(witness: &RawWitness) -> Vec<u8> {
    // RISC Zero env expects sequential reads via env::read_slice:
    // Same wire format as SP1 for now.
    let mut buf = Vec::new();
    buf.extend_from_slice(&(witness.boot_info.len() as u32).to_le_bytes());
    buf.extend_from_slice(&witness.boot_info);
    buf.extend_from_slice(&(witness.oracle_data.len() as u32).to_le_bytes());
    buf.extend_from_slice(&witness.oracle_data);
    buf.extend_from_slice(&(witness.blob_data.len() as u32).to_le_bytes());
    buf.extend_from_slice(&witness.blob_data);
    buf
}

/// Decode adapter bytes back into a RawWitness.
///
/// Inverse of both `raw_witness_to_sp1_bytes` and `raw_witness_to_risczero_bytes`.
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
            boot_info: b"boot-data-123".to_vec(),
            oracle_data: b"oracle-preimages".to_vec(),
            blob_data: b"blob-sidecar".to_vec(),
        }
    }

    #[test]
    fn bytes_roundtrip() {
        let witness = sample_witness();
        // Manually construct the wire format
        let mut buf = Vec::new();
        buf.extend_from_slice(&(witness.boot_info.len() as u32).to_le_bytes());
        buf.extend_from_slice(&witness.boot_info);
        buf.extend_from_slice(&(witness.oracle_data.len() as u32).to_le_bytes());
        buf.extend_from_slice(&witness.oracle_data);
        buf.extend_from_slice(&(witness.blob_data.len() as u32).to_le_bytes());
        buf.extend_from_slice(&witness.blob_data);

        let decoded = bytes_to_raw_witness(&buf).unwrap();
        assert_eq!(decoded.boot_info, witness.boot_info);
        assert_eq!(decoded.oracle_data, witness.oracle_data);
        assert_eq!(decoded.blob_data, witness.blob_data);
    }

    #[test]
    fn empty_witness_roundtrip() {
        let _witness = RawWitness {
            boot_info: vec![],
            oracle_data: vec![],
            blob_data: vec![],
        };
        let mut buf = Vec::new();
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());

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
    fn sp1_adapter_roundtrip() {
        let witness = sample_witness();
        let buf = raw_witness_to_sp1_bytes(&witness);
        let decoded = bytes_to_raw_witness(&buf).unwrap();
        assert_eq!(decoded.boot_info, witness.boot_info);
        assert_eq!(decoded.oracle_data, witness.oracle_data);
        assert_eq!(decoded.blob_data, witness.blob_data);
    }

    #[cfg(feature = "risczero")]
    #[test]
    fn risczero_adapter_roundtrip() {
        let witness = sample_witness();
        let buf = raw_witness_to_risczero_bytes(&witness);
        let decoded = bytes_to_raw_witness(&buf).unwrap();
        assert_eq!(decoded.boot_info, witness.boot_info);
        assert_eq!(decoded.oracle_data, witness.oracle_data);
        assert_eq!(decoded.blob_data, witness.blob_data);
    }
}
