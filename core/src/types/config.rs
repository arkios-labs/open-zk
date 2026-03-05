use serde::{Deserialize, Serialize};

/// Proof mode: how the rollup achieves finality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProofMode {
    /// Beacon: every output root submission requires a ZK proof. Instant finality.
    Beacon,
    /// Sentinel: ZK proof used to resolve disputes. Hybrid model.
    Sentinel,
}

/// Security level hint for the intent resolver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityLevel {
    /// Maximum security — small aggregation windows, always Beacon.
    Maximum,
    /// Standard — balanced cost vs finality.
    Standard,
    /// Economy — large aggregation windows, cost-optimized.
    Economy,
}
