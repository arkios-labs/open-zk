use crate::types::ProofRequest;
use async_trait::async_trait;

/// Raw witness data generated for a proof request.
///
/// Backend-agnostic: contains all oracle data needed to replay
/// the OP Stack derivation pipeline inside a zkVM guest.
#[derive(Debug, Clone)]
pub struct RawWitness {
    /// Serialized boot info (L1 head, L2 output roots, block range).
    pub boot_info: Vec<u8>,
    /// Preimage oracle key-value pairs (state, code, preimages).
    pub oracle_data: Vec<u8>,
    /// Blob data from L1 beacon chain.
    pub blob_data: Vec<u8>,
}

/// Generates witness data for a given proof request.
///
/// The witness provider fetches all L1/L2 data needed to replay
/// the state transition inside a zkVM guest program.
#[async_trait]
pub trait WitnessProvider: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn generate_witness(&self, request: &ProofRequest) -> Result<RawWitness, Self::Error>;
}
