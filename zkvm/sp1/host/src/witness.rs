use crate::prover::Sp1Witness;
use open_zk_core::traits::RawWitness;
use sp1_sdk::SP1Stdin;

/// Convert a RawWitness into an SP1 witness.
///
/// The oracle_data already contains all preimages (including boot info as
/// local preimage keys), so we write it as a single `Vec<u8>`.
pub fn raw_witness_to_sp1_witness(witness: &RawWitness) -> Result<Sp1Witness, String> {
    let mut stdin = SP1Stdin::new();
    stdin.write(&witness.oracle_data);

    Ok(Sp1Witness { stdin })
}
