use open_zk_core::types::{ProofMode, ProvingMode, SecurityLevel, ZkvmBackend};
use std::time::Duration;

/// Resolved intent: concrete decisions from user-declared constraints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedIntent {
    pub proof_mode: ProofMode,
    pub backend: ZkvmBackend,
    pub proving_mode: ProvingMode,
    pub aggregation_window: u64,
}

/// Resolves high-level user intent into concrete proving parameters.
pub struct IntentResolver;

impl IntentResolver {
    /// Resolve user-declared constraints into a concrete proving plan.
    ///
    /// Rules:
    /// - finality < 30min + budget ok → Beacon + SP1
    /// - finality < 30min + budget tight → Beacon + RISC Zero
    /// - finality > 1hr → Sentinel + cost-optimized
    /// - security = Maximum → always Beacon
    /// - security = Economy → Sentinel + RISC Zero
    pub fn resolve(
        target_finality: Duration,
        max_cost_per_proof: f64,
        security: SecurityLevel,
    ) -> ResolvedIntent {
        let (proof_mode, backend) = match security {
            SecurityLevel::Maximum => (ProofMode::Beacon, ZkvmBackend::Sp1),
            SecurityLevel::Economy => (ProofMode::Sentinel, ZkvmBackend::RiscZero),
            SecurityLevel::Standard => {
                if target_finality <= Duration::from_secs(30 * 60) {
                    if max_cost_per_proof >= 0.50 {
                        (ProofMode::Beacon, ZkvmBackend::Sp1)
                    } else {
                        (ProofMode::Beacon, ZkvmBackend::RiscZero)
                    }
                } else {
                    (ProofMode::Sentinel, ZkvmBackend::RiscZero)
                }
            }
        };

        let aggregation_window = match security {
            SecurityLevel::Maximum => 10,
            SecurityLevel::Standard => 100,
            SecurityLevel::Economy => 1000,
        };

        ResolvedIntent {
            proof_mode,
            backend,
            proving_mode: ProvingMode::Groth16,
            aggregation_window,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_security_always_beacon_sp1() {
        let result =
            IntentResolver::resolve(Duration::from_secs(3600), 0.10, SecurityLevel::Maximum);
        assert_eq!(result.proof_mode, ProofMode::Beacon);
        assert_eq!(result.backend, ZkvmBackend::Sp1);
        assert_eq!(result.aggregation_window, 10);
    }

    #[test]
    fn economy_always_sentinel_risczero() {
        let result = IntentResolver::resolve(Duration::from_secs(60), 10.0, SecurityLevel::Economy);
        assert_eq!(result.proof_mode, ProofMode::Sentinel);
        assert_eq!(result.backend, ZkvmBackend::RiscZero);
        assert_eq!(result.aggregation_window, 1000);
    }

    #[test]
    fn standard_fast_finality_high_budget_beacon_sp1() {
        let result = IntentResolver::resolve(
            Duration::from_secs(600), // 10 min
            0.50,
            SecurityLevel::Standard,
        );
        assert_eq!(result.proof_mode, ProofMode::Beacon);
        assert_eq!(result.backend, ZkvmBackend::Sp1);
        assert_eq!(result.aggregation_window, 100);
    }

    #[test]
    fn standard_fast_finality_low_budget_beacon_risczero() {
        let result = IntentResolver::resolve(
            Duration::from_secs(600), // 10 min
            0.10,
            SecurityLevel::Standard,
        );
        assert_eq!(result.proof_mode, ProofMode::Beacon);
        assert_eq!(result.backend, ZkvmBackend::RiscZero);
    }

    #[test]
    fn standard_slow_finality_sentinel() {
        let result = IntentResolver::resolve(
            Duration::from_secs(7200), // 2 hours
            1.0,
            SecurityLevel::Standard,
        );
        assert_eq!(result.proof_mode, ProofMode::Sentinel);
        assert_eq!(result.backend, ZkvmBackend::RiscZero);
    }
}
