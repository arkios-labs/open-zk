//! Witness adapter utilities for converting RawWitness to backend-specific formats.

use open_zk_core::traits::RawWitness;

#[cfg(feature = "sp1")]
pub use open_zk_zkvm_sp1_host::raw_witness_to_sp1_witness;

#[cfg(feature = "risc0")]
pub use open_zk_zkvm_risc0_host::raw_witness_to_risc0_witness;

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
}
