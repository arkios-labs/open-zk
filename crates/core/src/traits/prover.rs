use crate::types::{CostEstimate, ProofArtifact, ProvingMode};
use async_trait::async_trait;

/// A program that can be executed inside a zkVM.
pub trait GuestProgram: Send + Sync {
    fn program_id(&self) -> &[u8];
    fn name(&self) -> &str;
}

/// Input witness data for a proof request.
pub trait WitnessInput: Send + Sync {}

/// Backend-agnostic prover interface.
#[async_trait]
pub trait ProverBackend: Send + Sync {
    type Witness: WitnessInput;
    type Program: GuestProgram;
    type Error: std::error::Error + Send + Sync + 'static;

    fn name(&self) -> &str;

    async fn prove(
        &self,
        program: &Self::Program,
        witness: &Self::Witness,
        mode: ProvingMode,
    ) -> Result<ProofArtifact, Self::Error>;

    async fn verify(
        &self,
        program: &Self::Program,
        proof: &ProofArtifact,
    ) -> Result<bool, Self::Error>;

    async fn estimate_cost(
        &self,
        program: &Self::Program,
        witness: &Self::Witness,
    ) -> Result<CostEstimate, Self::Error>;
}
