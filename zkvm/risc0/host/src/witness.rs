use crate::prover::RiscZeroWitness;
use open_zk_core::traits::RawWitness;

/// Convert a RawWitness into a RISC Zero witness.
///
/// The oracle_data already contains all preimages (including boot info as
/// local preimage keys), so we just pass it through as-is.
pub fn raw_witness_to_risc0_witness(witness: &RawWitness) -> Result<RiscZeroWitness, String> {
    Ok(RiscZeroWitness {
        oracle_data: witness.oracle_data.clone(),
    })
}
