use open_zk_core::traits::{RawWitness, WitnessInput};
use risc0_zkvm::ExecutorEnv;

use crate::RiscZeroProverError;

/// Witness carrying serialized oracle data for RISC Zero guest execution.
///
/// The oracle_data contains rkyv-serialized preimages (including boot info
/// as local preimage keys). The guest reads a single `Vec<u8>` from stdin.
pub struct RiscZeroWitness {
    /// Serialized oracle preimage data (rkyv BTreeMap) for the guest pipeline.
    pub oracle_data: Vec<u8>,
}

impl WitnessInput for RiscZeroWitness {}

impl RiscZeroWitness {
    /// Build a RISC Zero `ExecutorEnv` from the oracle data.
    ///
    /// Writes a single `Vec<u8>` matching the guest's `io.read::<Vec<u8>>()`.
    pub(crate) fn build_env(&self) -> Result<ExecutorEnv<'static>, RiscZeroProverError> {
        ExecutorEnv::builder()
            .write(&self.oracle_data)
            .map_err(|e| RiscZeroProverError::ProvingFailed(format!("write oracle_data: {e}")))?
            .build()
            .map_err(|e| RiscZeroProverError::ProvingFailed(e.to_string()))
    }
}

/// Convert a RawWitness into a RISC Zero witness.
///
/// The oracle_data already contains all preimages (including boot info as
/// local preimage keys), so we just pass it through as-is.
pub fn raw_witness_to_risc0_witness(witness: &RawWitness) -> Result<RiscZeroWitness, String> {
    Ok(RiscZeroWitness {
        oracle_data: witness.oracle_data.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risc0_witness_from_raw() {
        let witness = RawWitness {
            boot_info: vec![],
            oracle_data: b"oracle-preimages".to_vec(),
            blob_data: vec![],
        };
        let rz_witness = raw_witness_to_risc0_witness(&witness).unwrap();
        assert_eq!(rz_witness.oracle_data, b"oracle-preimages");
    }
}
