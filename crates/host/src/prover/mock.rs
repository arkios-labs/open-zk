use async_trait::async_trait;
use open_zk_core::traits::{GuestProgram, ProverBackend, WitnessInput};
use open_zk_core::types::{CostEstimate, ProofArtifact, ProvingMode, ZkvmBackend};

/// Mock witness that carries raw bytes for testing.
#[derive(Debug, Default)]
pub struct MockWitness {
    pub data: Vec<u8>,
}

impl WitnessInput for MockWitness {}

/// Mock guest program for testing.
#[derive(Debug, Clone)]
pub struct MockProgram {
    pub id: Vec<u8>,
    pub program_name: String,
}

impl MockProgram {
    pub fn new(name: &str) -> Self {
        Self {
            id: name.as_bytes().to_vec(),
            program_name: name.to_string(),
        }
    }
}

impl GuestProgram for MockProgram {
    fn program_id(&self) -> &[u8] {
        &self.id
    }
    fn name(&self) -> &str {
        &self.program_name
    }
}

/// A prover backend that executes nothing and returns dummy proofs.
/// Useful for testing the orchestration layer without a real zkVM.
#[derive(Debug, Default)]
pub struct MockProverBackend;

#[derive(Debug, thiserror::Error)]
#[error("mock prover error: {0}")]
pub struct MockProverError(pub String);

#[async_trait]
impl ProverBackend for MockProverBackend {
    type Witness = MockWitness;
    type Program = MockProgram;
    type Error = MockProverError;

    fn name(&self) -> &str {
        "mock"
    }

    async fn prove(
        &self,
        _program: &Self::Program,
        _witness: &Self::Witness,
        mode: ProvingMode,
    ) -> Result<ProofArtifact, Self::Error> {
        Ok(ProofArtifact {
            backend: ZkvmBackend::Mock,
            mode,
            proof_bytes: vec![0xDE, 0xAD],
            public_values: vec![],
            program_id: alloy_primitives::B256::ZERO,
            cycle_count: Some(0),
        })
    }

    async fn verify(
        &self,
        _program: &Self::Program,
        proof: &ProofArtifact,
    ) -> Result<bool, Self::Error> {
        Ok(proof.backend == ZkvmBackend::Mock)
    }

    async fn estimate_cost(
        &self,
        _program: &Self::Program,
        _witness: &Self::Witness,
    ) -> Result<CostEstimate, Self::Error> {
        Ok(CostEstimate {
            estimated_cycles: 0,
            estimated_cost_usd: 0.0,
            estimated_duration_secs: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_prove_and_verify() {
        let backend = MockProverBackend;
        let program = MockProgram::new("test-range");
        let witness = MockWitness::default();

        let proof = backend
            .prove(&program, &witness, ProvingMode::Execute)
            .await
            .unwrap();
        assert_eq!(proof.backend, ZkvmBackend::Mock);
        assert_eq!(proof.mode, ProvingMode::Execute);

        let valid = backend.verify(&program, &proof).await.unwrap();
        assert!(valid);
    }

    #[tokio::test]
    async fn mock_estimate_cost_is_zero() {
        let backend = MockProverBackend;
        let program = MockProgram::new("test");
        let witness = MockWitness::default();

        let estimate = backend.estimate_cost(&program, &witness).await.unwrap();
        assert_eq!(estimate.estimated_cost_usd, 0.0);
    }
}
