use alloy_primitives::B256;
use serde::{Deserialize, Serialize};

/// Which zkVM backend produced (or should produce) the proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ZkvmBackend {
    Sp1,
    RiscZero,
    /// Dynamically select the optimal backend based on cost, latency, and
    /// availability. Reserved for future implementation — currently resolves
    /// to SP1 as the default.
    Auto,
    Mock,
}

/// The proving execution mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProvingMode {
    /// Full ZK proof (Groth16 / STARK-to-SNARK).
    Groth16,
    /// Compressed proof (cheaper but not on-chain verifiable in some backends).
    Compressed,
    /// Execute only — no proof generated. For testing.
    Execute,
}

/// A completed proof with its metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofArtifact {
    pub backend: ZkvmBackend,
    pub mode: ProvingMode,
    pub proof_bytes: Vec<u8>,
    pub public_values: Vec<u8>,
    pub program_id: B256,
    pub cycle_count: Option<u64>,
}

/// Request to generate a proof for a range of L2 blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofRequest {
    /// L1 block hash as trust anchor.
    pub l1_head: B256,
    /// Starting L2 block number (inclusive).
    pub l2_start_block: u64,
    /// Ending L2 block number (inclusive).
    pub l2_end_block: u64,
    /// L2 output root at `l2_start_block`.
    pub l2_start_output_root: B256,
    /// Desired proving mode.
    pub mode: ProvingMode,
}

/// Cost estimate returned by a prover backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    pub estimated_cycles: u64,
    pub estimated_cost_usd: f64,
    pub estimated_duration_secs: u64,
}

/// A plan for proving a range, potentially split into sub-ranges + aggregation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofPlan {
    pub backend: ZkvmBackend,
    pub mode: ProvingMode,
    /// Individual range proof requests.
    pub range_requests: Vec<ProofRequest>,
    /// Whether aggregation is needed after range proofs complete.
    pub needs_aggregation: bool,
    pub estimated_total_cost: CostEstimate,
}
