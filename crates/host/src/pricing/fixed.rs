use async_trait::async_trait;
use open_zk_core::traits::PricingProvider;
use open_zk_core::types::{CycleEstimate, PricingInfo, ZkvmBackend};

/// Fixed-rate pricing using hardcoded per-cycle rates.
///
/// Default rates:
/// - SP1: $0.01 / 10M cycles, 1M cycles/sec
/// - RISC Zero: $0.008 / 10M cycles, 500K cycles/sec
/// - Mock: $0.0, 1 cycle/sec
pub struct FixedPricing {
    sp1_cost_per_10m: f64,
    sp1_cycles_per_sec: u64,
    risc0_cost_per_10m: f64,
    risc0_cycles_per_sec: u64,
}

impl Default for FixedPricing {
    fn default() -> Self {
        Self {
            sp1_cost_per_10m: 0.01,
            sp1_cycles_per_sec: 1_000_000,
            risc0_cost_per_10m: 0.008,
            risc0_cycles_per_sec: 500_000,
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("fixed pricing error: {0}")]
pub struct FixedPricingError(pub String);

#[async_trait]
impl PricingProvider for FixedPricing {
    type Error = FixedPricingError;

    fn name(&self) -> &str {
        "fixed"
    }

    fn supports_backend(&self, _backend: ZkvmBackend) -> bool {
        true
    }

    async fn price(&self, estimate: &CycleEstimate) -> Result<PricingInfo, Self::Error> {
        let (cost_usd, duration_secs) = match estimate.backend {
            ZkvmBackend::Sp1 => (
                (estimate.cycles as f64) / 10_000_000.0 * self.sp1_cost_per_10m,
                estimate.cycles / self.sp1_cycles_per_sec,
            ),
            ZkvmBackend::RiscZero => (
                (estimate.cycles as f64) / 10_000_000.0 * self.risc0_cost_per_10m,
                estimate.cycles / self.risc0_cycles_per_sec,
            ),
            ZkvmBackend::Mock | ZkvmBackend::Auto => (0.0, 0),
        };

        Ok(PricingInfo {
            cost_usd,
            duration_secs,
            source: "fixed".to_string(),
            native_cost: None,
            native_symbol: None,
            token_usd_rate: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fixed_pricing_sp1() {
        let pricing = FixedPricing::default();
        let estimate = CycleEstimate {
            cycles: 10_000_000,
            backend: ZkvmBackend::Sp1,
        };
        let info = pricing.price(&estimate).await.unwrap();
        assert!((info.cost_usd - 0.01).abs() < 1e-10);
        assert_eq!(info.duration_secs, 10);
        assert_eq!(info.source, "fixed");
    }

    #[tokio::test]
    async fn fixed_pricing_risc_zero() {
        let pricing = FixedPricing::default();
        let estimate = CycleEstimate {
            cycles: 10_000_000,
            backend: ZkvmBackend::RiscZero,
        };
        let info = pricing.price(&estimate).await.unwrap();
        assert!((info.cost_usd - 0.008).abs() < 1e-10);
        assert_eq!(info.duration_secs, 20);
        assert_eq!(info.source, "fixed");
    }

    #[tokio::test]
    async fn fixed_pricing_mock() {
        let pricing = FixedPricing::default();
        let estimate = CycleEstimate {
            cycles: 0,
            backend: ZkvmBackend::Mock,
        };
        let info = pricing.price(&estimate).await.unwrap();
        assert_eq!(info.cost_usd, 0.0);
        assert_eq!(info.duration_secs, 0);
    }
}
