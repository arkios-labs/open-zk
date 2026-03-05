use open_zk_core::traits::{RawWitness, WitnessInput};
use sp1_sdk::SP1Stdin;

/// Witness carrying SP1-formatted stdin data.
pub struct Sp1Witness {
    pub stdin: SP1Stdin,
}

impl WitnessInput for Sp1Witness {}

/// Convert a RawWitness into an SP1 witness.
///
/// The oracle_data already contains all preimages (including boot info as
/// local preimage keys), so we write it as a single `Vec<u8>`.
pub fn raw_witness_to_sp1_witness(witness: &RawWitness) -> Result<Sp1Witness, String> {
    let mut stdin = SP1Stdin::new();
    stdin.write(&witness.oracle_data);

    Ok(Sp1Witness { stdin })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sp1_witness_from_raw() {
        let witness = RawWitness {
            boot_info: vec![],
            oracle_data: b"oracle-preimages".to_vec(),
            blob_data: vec![],
        };
        let sp1_witness = raw_witness_to_sp1_witness(&witness).unwrap();
        let _ = sp1_witness;
    }
}
