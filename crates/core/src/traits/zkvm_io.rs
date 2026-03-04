use serde::{de::DeserializeOwned, Serialize};

/// Read inputs inside a zkVM guest program.
pub trait ZkVmReader {
    fn read<T: DeserializeOwned>(&self) -> T;
    fn read_slice(&self, buf: &mut [u8]);
}

/// Commit outputs from a zkVM guest program.
pub trait ZkVmWriter {
    fn commit<T: Serialize>(&self, value: &T);
    fn commit_slice(&self, data: &[u8]);
}

/// Compose proofs by verifying inner proofs within a guest.
pub trait ZkVmComposer {
    type ProgramId;
    fn verify_inner_proof(&self, program_id: &Self::ProgramId, public_values: &[u8]);
}
