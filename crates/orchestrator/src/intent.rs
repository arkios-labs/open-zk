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
///
/// Separation of concerns:
/// - `security` + `target_finality` → proof mode (Beacon/Sentinel) + aggregation window
/// - `backend` → which zkVM to use (independent of security level)
///
/// When `backend = Auto`, the first entry in `allowed_backends` is used.
/// Future versions will select dynamically based on cost/latency/availability.
pub struct IntentResolver;

impl IntentResolver {
    /// Default allowed backends when none are specified.
    pub const DEFAULT_ALLOWED_BACKENDS: &[ZkvmBackend] = &[ZkvmBackend::Sp1, ZkvmBackend::RiscZero];

    /// Resolve user-declared constraints into a concrete proving plan.
    pub fn resolve(
        backend: ZkvmBackend,
        allowed_backends: &[ZkvmBackend],
        target_finality: Duration,
        security: SecurityLevel,
    ) -> ResolvedIntent {
        // 1. Security + finality → proof mode
        let proof_mode = Self::resolve_proof_mode(target_finality, security);

        // 2. Security → aggregation window
        let aggregation_window = match security {
            SecurityLevel::Maximum => 10,
            SecurityLevel::Standard => 100,
            SecurityLevel::Economy => 1000,
        };

        // 3. Backend selection (independent of security)
        let resolved_backend = match backend {
            ZkvmBackend::Auto => Self::resolve_auto(allowed_backends),
            explicit => explicit,
        };

        ResolvedIntent {
            proof_mode,
            backend: resolved_backend,
            proving_mode: ProvingMode::Groth16,
            aggregation_window,
        }
    }

    /// Derive proof mode from security level and target finality.
    fn resolve_proof_mode(target_finality: Duration, security: SecurityLevel) -> ProofMode {
        match security {
            SecurityLevel::Maximum => ProofMode::Beacon,
            SecurityLevel::Economy => ProofMode::Sentinel,
            SecurityLevel::Standard => {
                if target_finality <= Duration::from_secs(30 * 60) {
                    ProofMode::Beacon
                } else {
                    ProofMode::Sentinel
                }
            }
        }
    }

    /// Select backend from allowed list.
    ///
    /// Currently picks the first entry. Future: integrate pricing APIs
    /// for dynamic cost/latency-based selection.
    fn resolve_auto(allowed_backends: &[ZkvmBackend]) -> ZkvmBackend {
        allowed_backends
            .first()
            .copied()
            .unwrap_or(ZkvmBackend::Sp1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_sp1_beacon() {
        let result = IntentResolver::resolve(
            ZkvmBackend::Sp1,
            &[],
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
            &[],
            Duration::from_secs(7200),
            SecurityLevel::Standard,
        );
        assert_eq!(result.backend, ZkvmBackend::RiscZero);
        assert_eq!(result.proof_mode, ProofMode::Sentinel);
    }

    #[test]
    fn auto_picks_first_allowed() {
        let result = IntentResolver::resolve(
            ZkvmBackend::Auto,
            &[ZkvmBackend::RiscZero, ZkvmBackend::Sp1],
            Duration::from_secs(600),
            SecurityLevel::Standard,
        );
        assert_eq!(result.backend, ZkvmBackend::RiscZero);
    }

    #[test]
    fn auto_defaults_to_sp1_when_empty() {
        let result = IntentResolver::resolve(
            ZkvmBackend::Auto,
            &[],
            Duration::from_secs(600),
            SecurityLevel::Standard,
        );
        assert_eq!(result.backend, ZkvmBackend::Sp1);
    }

    #[test]
    fn security_maximum_always_beacon() {
        let result = IntentResolver::resolve(
            ZkvmBackend::Sp1,
            &[],
            Duration::from_secs(7200),
            SecurityLevel::Maximum,
        );
        assert_eq!(result.proof_mode, ProofMode::Beacon);
        assert_eq!(result.aggregation_window, 10);
    }

    #[test]
    fn security_economy_always_sentinel() {
        let result = IntentResolver::resolve(
            ZkvmBackend::Sp1,
            &[],
            Duration::from_secs(60),
            SecurityLevel::Economy,
        );
        assert_eq!(result.proof_mode, ProofMode::Sentinel);
        assert_eq!(result.aggregation_window, 1000);
    }

    #[test]
    fn standard_fast_finality_beacon() {
        let result = IntentResolver::resolve(
            ZkvmBackend::Auto,
            IntentResolver::DEFAULT_ALLOWED_BACKENDS,
            Duration::from_secs(600),
            SecurityLevel::Standard,
        );
        assert_eq!(result.proof_mode, ProofMode::Beacon);
        assert_eq!(result.backend, ZkvmBackend::Sp1);
        assert_eq!(result.aggregation_window, 100);
    }

    #[test]
    fn standard_slow_finality_sentinel() {
        let result = IntentResolver::resolve(
            ZkvmBackend::Auto,
            IntentResolver::DEFAULT_ALLOWED_BACKENDS,
            Duration::from_secs(7200),
            SecurityLevel::Standard,
        );
        assert_eq!(result.proof_mode, ProofMode::Sentinel);
        assert_eq!(result.backend, ZkvmBackend::Sp1);
    }

    #[test]
    fn mock_backend_respects_security() {
        let result = IntentResolver::resolve(
            ZkvmBackend::Mock,
            &[],
            Duration::from_secs(7200),
            SecurityLevel::Economy,
        );
        assert_eq!(result.backend, ZkvmBackend::Mock);
        assert_eq!(result.proof_mode, ProofMode::Sentinel);
    }
}
