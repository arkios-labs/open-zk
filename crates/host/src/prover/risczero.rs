use alloy_primitives::B256;
use async_trait::async_trait;
use open_zk_core::traits::{GuestProgram, ProverBackend, WitnessInput};
use open_zk_core::types::{CostEstimate, ProofArtifact, ProvingMode, ZkvmBackend};
use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts, Receipt};

/// Witness carrying RISC Zero executor environment inputs.
pub struct RiscZeroWitness {
    pub input_data: Vec<u8>,
}

impl WitnessInput for RiscZeroWitness {}

/// A guest program identified by its RISC Zero image ID and ELF.
pub struct RiscZeroProgram {
    pub image_id: [u32; 8],
    pub elf: Vec<u8>,
    pub program_name: String,
}

impl RiscZeroProgram {
    pub fn new(name: &str, image_id: [u32; 8], elf: Vec<u8>) -> Self {
        Self {
            image_id,
            elf,
            program_name: name.to_string(),
        }
    }
}

impl GuestProgram for RiscZeroProgram {
    fn program_id(&self) -> &[u8] {
        bytemuck::cast_slice(&self.image_id)
    }

    fn name(&self) -> &str {
        &self.program_name
    }
}

/// RISC Zero prover backend.
///
/// Supports local proving and remote proving via Bonsai.
pub struct RiscZeroProverBackend;

#[derive(Debug, thiserror::Error)]
pub enum RiscZeroProverError {
    #[error("risc0 proving failed: {0}")]
    ProvingFailed(String),
    #[error("risc0 verification failed: {0}")]
    VerificationFailed(String),
}

impl RiscZeroProverBackend {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProverBackend for RiscZeroProverBackend {
    type Witness = RiscZeroWitness;
    type Program = RiscZeroProgram;
    type Error = RiscZeroProverError;

    fn name(&self) -> &str {
        "risczero"
    }

    async fn prove(
        &self,
        program: &Self::Program,
        witness: &Self::Witness,
        mode: ProvingMode,
    ) -> Result<ProofArtifact, Self::Error> {
        let env = ExecutorEnv::builder()
            .write_slice(&witness.input_data)
            .build()
            .map_err(|e| RiscZeroProverError::ProvingFailed(e.to_string()))?;

        let prover = default_prover();

        let opts = match mode {
            ProvingMode::Execute => {
                // Execute-only: run without generating a proof.
                let session = risc0_zkvm::ExecutorImpl::from_elf(env, &program.elf)
                    .map_err(|e| RiscZeroProverError::ProvingFailed(e.to_string()))?
                    .run()
                    .map_err(|e| RiscZeroProverError::ProvingFailed(e.to_string()))?;

                return Ok(ProofArtifact {
                    backend: ZkvmBackend::RiscZero,
                    mode,
                    proof_bytes: vec![],
                    public_values: session.journal.bytes.clone(),
                    program_id: B256::from_slice(bytemuck::cast_slice(&program.image_id)),
                    cycle_count: Some(session.segments.len() as u64 * 1_048_576),
                });
            }
            ProvingMode::Compressed => ProverOpts::succinct(),
            ProvingMode::Groth16 => ProverOpts::groth16(),
        };

        let receipt = prover
            .prove_with_opts(env, &program.elf, &opts)
            .map_err(|e| RiscZeroProverError::ProvingFailed(e.to_string()))?
            .receipt;

        let journal_bytes = receipt.journal.bytes.clone();
        let receipt_bytes = bincode::serialize(&receipt)
            .map_err(|e| RiscZeroProverError::ProvingFailed(e.to_string()))?;

        Ok(ProofArtifact {
            backend: ZkvmBackend::RiscZero,
            mode,
            proof_bytes: receipt_bytes,
            public_values: journal_bytes,
            program_id: B256::from_slice(bytemuck::cast_slice(&program.image_id)),
            cycle_count: None,
        })
    }

    async fn verify(
        &self,
        program: &Self::Program,
        proof: &ProofArtifact,
    ) -> Result<bool, Self::Error> {
        if proof.mode == ProvingMode::Execute {
            return Ok(true);
        }

        let receipt: Receipt = bincode::deserialize(&proof.proof_bytes)
            .map_err(|e| RiscZeroProverError::VerificationFailed(e.to_string()))?;

        receipt
            .verify(program.image_id)
            .map_err(|e| RiscZeroProverError::VerificationFailed(e.to_string()))?;

        Ok(true)
    }

    async fn estimate_cost(
        &self,
        program: &Self::Program,
        witness: &Self::Witness,
    ) -> Result<CostEstimate, Self::Error> {
        let env = ExecutorEnv::builder()
            .write_slice(&witness.input_data)
            .build()
            .map_err(|e| RiscZeroProverError::ProvingFailed(e.to_string()))?;

        let session = risc0_zkvm::ExecutorImpl::from_elf(env, &program.elf)
            .map_err(|e| RiscZeroProverError::ProvingFailed(e.to_string()))?
            .run()
            .map_err(|e| RiscZeroProverError::ProvingFailed(e.to_string()))?;

        let segments = session.segments.len() as u64;
        let cycles = segments * 1_048_576; // 2^20 cycles per segment

        Ok(CostEstimate {
            estimated_cycles: cycles,
            // Rough estimate: Bonsai/Boundless pricing
            estimated_cost_usd: (cycles as f64) / 10_000_000.0 * 0.008,
            estimated_duration_secs: cycles / 500_000,
        })
    }
}
