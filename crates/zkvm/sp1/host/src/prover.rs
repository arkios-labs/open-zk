use alloy_primitives::B256;
use async_trait::async_trait;
use open_zk_core::traits::{GuestProgram, ProverBackend};
use open_zk_core::types::{CostEstimate, ProofArtifact, ProvingMode, ZkvmBackend};
use sp1_sdk::{Elf, HashableKey, ProveRequest, Prover, ProverClient, ProvingKey};

use crate::Sp1Witness;

/// A guest program identified by its compiled ELF binary.
pub struct Sp1Program {
    pub elf: Vec<u8>,
    pub program_name: String,
}

impl Sp1Program {
    pub fn new(name: &str, elf: Vec<u8>) -> Self {
        Self {
            elf,
            program_name: name.to_string(),
        }
    }

    fn to_elf(&self) -> Elf {
        Elf::from(std::sync::Arc::from(self.elf.as_slice()))
    }
}

impl GuestProgram for Sp1Program {
    fn program_id(&self) -> &[u8] {
        &self.elf
    }

    fn name(&self) -> &str {
        &self.program_name
    }
}

/// SP1 prover backend using the Succinct SDK v6.
///
/// Supports local proving (CPU/CUDA/mock) and remote proving via the Succinct Network.
/// The prover type is determined by the `SP1_PROVER` env var (default: "cpu").
/// Set `SP1_PROVER=mock` for fast execution without real ZK proof generation.
pub struct Sp1ProverBackend {
    client: sp1_sdk::env::EnvProver,
}

#[derive(Debug, thiserror::Error)]
pub enum Sp1ProverError {
    #[error("sp1 proving failed: {0}")]
    ProvingFailed(String),
    #[error("sp1 verification failed: {0}")]
    VerificationFailed(String),
}

impl Sp1ProverBackend {
    pub async fn new() -> Self {
        Self {
            client: ProverClient::from_env().await,
        }
    }
}

#[async_trait]
impl ProverBackend for Sp1ProverBackend {
    type Witness = Sp1Witness;
    type Program = Sp1Program;
    type Error = Sp1ProverError;

    fn name(&self) -> &str {
        "sp1"
    }

    async fn prove(
        &self,
        program: &Self::Program,
        witness: &Self::Witness,
        mode: ProvingMode,
    ) -> Result<ProofArtifact, Self::Error> {
        let elf = program.to_elf();

        match mode {
            ProvingMode::Execute => {
                let (public_values, report) = self
                    .client
                    .execute(elf, witness.stdin.clone())
                    .await
                    .map_err(|e| Sp1ProverError::ProvingFailed(e.to_string()))?;
                Ok(ProofArtifact {
                    backend: ZkvmBackend::Sp1,
                    mode,
                    proof_bytes: vec![],
                    public_values: public_values.to_vec(),
                    program_id: B256::ZERO,
                    cycle_count: Some(report.total_instruction_count()),
                })
            }
            ProvingMode::Compressed | ProvingMode::Groth16 => {
                let pk = self
                    .client
                    .setup(elf)
                    .await
                    .map_err(|e| Sp1ProverError::ProvingFailed(format!("setup: {e}")))?;

                let proof = match mode {
                    ProvingMode::Compressed => self
                        .client
                        .prove(&pk, witness.stdin.clone())
                        .compressed()
                        .await
                        .map_err(|e| Sp1ProverError::ProvingFailed(e.to_string()))?,
                    ProvingMode::Groth16 => self
                        .client
                        .prove(&pk, witness.stdin.clone())
                        .groth16()
                        .await
                        .map_err(|e| Sp1ProverError::ProvingFailed(e.to_string()))?,
                    _ => unreachable!(),
                };

                let public_values = proof.public_values.to_vec();
                let proof_bytes = bincode::serialize(&proof)
                    .map_err(|e| Sp1ProverError::ProvingFailed(e.to_string()))?;

                let vk_bytes = pk.verifying_key().bytes32_raw();
                Ok(ProofArtifact {
                    backend: ZkvmBackend::Sp1,
                    mode,
                    proof_bytes,
                    public_values,
                    program_id: B256::from(vk_bytes),
                    cycle_count: None,
                })
            }
        }
    }

    async fn verify(
        &self,
        program: &Self::Program,
        proof: &ProofArtifact,
    ) -> Result<bool, Self::Error> {
        if proof.mode == ProvingMode::Execute {
            return Ok(true);
        }

        let sp1_proof: sp1_sdk::SP1ProofWithPublicValues = bincode::deserialize(&proof.proof_bytes)
            .map_err(|e| Sp1ProverError::VerificationFailed(e.to_string()))?;

        let elf = program.to_elf();
        let pk = self
            .client
            .setup(elf)
            .await
            .map_err(|e| Sp1ProverError::VerificationFailed(format!("setup: {e}")))?;

        self.client
            .verify(&sp1_proof, pk.verifying_key(), None)
            .map_err(|e| Sp1ProverError::VerificationFailed(e.to_string()))?;

        Ok(true)
    }

    async fn estimate_cost(
        &self,
        program: &Self::Program,
        witness: &Self::Witness,
    ) -> Result<CostEstimate, Self::Error> {
        let elf = program.to_elf();

        let (_, report) = self
            .client
            .execute(elf, witness.stdin.clone())
            .await
            .map_err(|e| Sp1ProverError::ProvingFailed(e.to_string()))?;

        let cycles = report.total_instruction_count();

        Ok(CostEstimate {
            estimated_cycles: cycles,
            // Rough estimate: ~$0.01 per 10M cycles on Succinct Network
            estimated_cost_usd: (cycles as f64) / 10_000_000.0 * 0.01,
            estimated_duration_secs: cycles / 1_000_000, // ~1M cycles/sec
        })
    }
}
