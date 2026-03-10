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
    /// The `backend` parameter is the user's explicit choice:
    /// - `Sp1` / `RiscZero` / `Mock` — use as-is.
    /// - `Auto` — dynamically select based on `security` and `target_finality`.
    ///   Currently a placeholder; will integrate backend pricing APIs in the future.
    ///
    /// Rules for `Auto` resolution:
    /// - security = Maximum → Beacon + SP1
    /// - security = Economy → Sentinel + RISC Zero
    /// - security = Standard + finality ≤ 30min → Beacon + SP1
    /// - security = Standard + finality > 30min → Sentinel + RISC Zero
    pub fn resolve(
        backend: ZkvmBackend,
        target_finality: Duration,
        security: SecurityLevel,
    ) -> ResolvedIntent {
        let (proof_mode, resolved_backend) = match backend {
            ZkvmBackend::Auto => Self::resolve_auto(target_finality, security),
            ZkvmBackend::Mock => (ProofMode::Beacon, ZkvmBackend::Mock),
            explicit => {
                // User chose a specific backend; derive proof_mode from security/finality.
                let proof_mode = match security {
                    SecurityLevel::Maximum => ProofMode::Beacon,
                    SecurityLevel::Economy => ProofMode::Sentinel,
                    SecurityLevel::Standard => {
                        if target_finality <= Duration::from_secs(30 * 60) {
                            ProofMode::Beacon
                        } else {
                            ProofMode::Sentinel
                        }
                    }
                };
                (proof_mode, explicit)
            }
        };

        let aggregation_window = match security {
            SecurityLevel::Maximum => 10,
            SecurityLevel::Standard => 100,
            SecurityLevel::Economy => 1000,
        };

        ResolvedIntent {
            proof_mode,
            backend: resolved_backend,
            proving_mode: ProvingMode::Groth16,
            aggregation_window,
        }
    }

    /// Auto-select backend based on security level and target finality.
    ///
    /// Future: integrate with backend pricing APIs for real cost optimization.
    fn resolve_auto(
        target_finality: Duration,
        security: SecurityLevel,
    ) -> (ProofMode, ZkvmBackend) {
        match security {
            SecurityLevel::Maximum => (ProofMode::Beacon, ZkvmBackend::Sp1),
            SecurityLevel::Economy => (ProofMode::Sentinel, ZkvmBackend::RiscZero),
            SecurityLevel::Standard => {
                if target_finality <= Duration::from_secs(30 * 60) {
                    (ProofMode::Beacon, ZkvmBackend::Sp1)
                } else {
                    (ProofMode::Sentinel, ZkvmBackend::RiscZero)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_sp1_beacon() {
        let result = IntentResolver::resolve(
            ZkvmBackend::Sp1,
            Duration::from_secs(600),
            SecurityLevel::Standard,
        );
        assert_eq!(result.backend, ZkvmBackend::Sp1);
        assert_eq!(result.proof_mode, ProofMode::Beacon);
        assert_eq!(result.aggregation_window, 100);
    }

    #[test]
    fn explicit_risc0_sentinel() {
        let result = IntentResolver::resolve(
            ZkvmBackend::RiscZero,
            Duration::from_secs(7200),
            SecurityLevel::Standard,
        );
        assert_eq!(result.backend, ZkvmBackend::RiscZero);
        assert_eq!(result.proof_mode, ProofMode::Sentinel);
    }

    #[test]
    fn auto_max_security_beacon_sp1() {
        let result = IntentResolver::resolve(
            ZkvmBackend::Auto,
            Duration::from_secs(3600),
            SecurityLevel::Maximum,
        );
        assert_eq!(result.proof_mode, ProofMode::Beacon);
        assert_eq!(result.backend, ZkvmBackend::Sp1);
        assert_eq!(result.aggregation_window, 10);
    }

    #[test]
    fn auto_economy_sentinel_risc0() {
        let result = IntentResolver::resolve(
            ZkvmBackend::Auto,
            Duration::from_secs(60),
            SecurityLevel::Economy,
        );
        assert_eq!(result.proof_mode, ProofMode::Sentinel);
        assert_eq!(result.backend, ZkvmBackend::RiscZero);
        assert_eq!(result.aggregation_window, 1000);
    }

    #[test]
    fn auto_standard_fast_finality_beacon_sp1() {
        let result = IntentResolver::resolve(
            ZkvmBackend::Auto,
            Duration::from_secs(600),
            SecurityLevel::Standard,
        );
        assert_eq!(result.proof_mode, ProofMode::Beacon);
        assert_eq!(result.backend, ZkvmBackend::Sp1);
        assert_eq!(result.aggregation_window, 100);
    }

    #[test]
    fn auto_standard_slow_finality_sentinel_risc0() {
        let result = IntentResolver::resolve(
            ZkvmBackend::Auto,
            Duration::from_secs(7200),
            SecurityLevel::Standard,
        );
        assert_eq!(result.proof_mode, ProofMode::Sentinel);
        assert_eq!(result.backend, ZkvmBackend::RiscZero);
    }

    #[test]
    fn mock_backend_always_beacon() {
        let result = IntentResolver::resolve(
            ZkvmBackend::Mock,
            Duration::from_secs(7200),
            SecurityLevel::Economy,
        );
        assert_eq!(result.backend, ZkvmBackend::Mock);
        assert_eq!(result.proof_mode, ProofMode::Beacon);
    }
}
