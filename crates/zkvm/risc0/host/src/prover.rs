use alloy_primitives::B256;
use async_trait::async_trait;
use open_zk_core::traits::{GuestProgram, ProverBackend};
use open_zk_core::types::{CostEstimate, ProofArtifact, ProvingMode, ZkvmBackend};
use risc0_zkvm::{default_prover, ProverOpts, Receipt};

use crate::RiscZeroWitness;

/// Convert a RISC Zero image ID ([u32; 8]) to a 32-byte array (little-endian words).
fn image_id_to_bytes(image_id: &[u32; 8]) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    for (i, word) in image_id.iter().enumerate() {
        bytes[i * 4..(i + 1) * 4].copy_from_slice(&word.to_le_bytes());
    }
    bytes
}

/// A guest program identified by its RISC Zero image ID and ELF.
pub struct RiscZeroProgram {
    pub image_id: [u32; 8],
    image_id_bytes: [u8; 32],
    pub elf: Vec<u8>,
    pub program_name: String,
}

impl RiscZeroProgram {
    pub fn new(name: &str, image_id: [u32; 8], elf: Vec<u8>) -> Self {
        let image_id_bytes = image_id_to_bytes(&image_id);
        Self {
            image_id,
            image_id_bytes,
            elf,
            program_name: name.to_string(),
        }
    }
}

impl GuestProgram for RiscZeroProgram {
    fn program_id(&self) -> &[u8] {
        &self.image_id_bytes
    }

    fn name(&self) -> &str {
        &self.program_name
    }
}

/// RISC Zero prover backend.
///
/// Uses `RISC0_DEV_MODE=1` for fast execution without real ZK proof generation.
/// In dev mode, `default_prover()` produces fast dev receipts.
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
        "risc0"
    }

    async fn prove(
        &self,
        program: &Self::Program,
        witness: &Self::Witness,
        mode: ProvingMode,
    ) -> Result<ProofArtifact, Self::Error> {
        let env = witness.build_env()?;
        let prover = default_prover();
        let program_id = B256::from(program.image_id_bytes);

        let opts = match mode {
            ProvingMode::Execute => ProverOpts::default(),
            ProvingMode::Compressed => ProverOpts::succinct(),
            ProvingMode::Groth16 => ProverOpts::groth16(),
        };

        let prove_info = prover
            .prove_with_opts(env, &program.elf, &opts)
            .map_err(|e| RiscZeroProverError::ProvingFailed(e.to_string()))?;

        let journal_bytes = prove_info.receipt.journal.bytes.clone();
        let cycle_count = Some(prove_info.stats.total_cycles);

        if mode == ProvingMode::Execute {
            return Ok(ProofArtifact {
                backend: ZkvmBackend::RiscZero,
                mode,
                proof_bytes: vec![],
                public_values: journal_bytes,
                program_id,
                cycle_count,
            });
        }

        let receipt_bytes = bincode::serialize(&prove_info.receipt)
            .map_err(|e| RiscZeroProverError::ProvingFailed(e.to_string()))?;

        Ok(ProofArtifact {
            backend: ZkvmBackend::RiscZero,
            mode,
            proof_bytes: receipt_bytes,
            public_values: journal_bytes,
            program_id,
            cycle_count,
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
        let env = witness.build_env()?;
        let prover = default_prover();

        let prove_info = prover
            .prove(env, &program.elf)
            .map_err(|e| RiscZeroProverError::ProvingFailed(e.to_string()))?;

        let cycles = prove_info.stats.total_cycles;

        Ok(CostEstimate {
            estimated_cycles: cycles,
            estimated_cost_usd: (cycles as f64) / 10_000_000.0 * 0.008,
            estimated_duration_secs: cycles / 500_000,
        })
    }
}
