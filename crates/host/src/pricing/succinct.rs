use async_trait::async_trait;
use open_zk_core::traits::PricingProvider;
use open_zk_core::types::{CycleEstimate, PricingInfo, ZkvmBackend};
use sp1_sdk::network::{
    client::NetworkClient,
    proto::{types::FulfillmentStatus, GetFilteredProofRequestsResponse},
    signer::NetworkSigner,
    NetworkMode,
};
use tracing::warn;

const SP1_CYCLES_PER_SEC: u64 = 1_000_000;
const DEFAULT_SAMPLE_SIZE: u32 = 50;

/// Market-based pricing from the Succinct Prover Network.
///
/// Queries recent fulfilled proof requests via gRPC and computes
/// the median effective price per cycle (in PROVE tokens, 18 decimals).
///
/// Requires `NETWORK_PRIVATE_KEY` env var for gRPC authentication.
pub struct SuccinctPricing {
    client: NetworkClient,
    sample_size: u32,
    prove_usd: f64,
}

impl SuccinctPricing {
    /// Create from `NETWORK_PRIVATE_KEY` or `SP1_PRIVATE_KEY` env var.
    pub fn from_env(prove_usd: f64) -> Result<Self, SuccinctPricingError> {
        // Ensure a rustls CryptoProvider is installed (sp1-sdk uses aws-lc-rs).
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

        let private_key = std::env::var("NETWORK_PRIVATE_KEY")
            .or_else(|_| std::env::var("SP1_PRIVATE_KEY"))
            .map_err(|_| {
                SuccinctPricingError::Config(
                    "NETWORK_PRIVATE_KEY or SP1_PRIVATE_KEY env var required for Succinct pricing"
                        .to_string(),
                )
            })?;
        let signer = NetworkSigner::local(&private_key)
            .map_err(|e| SuccinctPricingError::Config(format!("invalid private key: {e}")))?;
        let client = NetworkClient::new(
            signer,
            "https://rpc.mainnet.succinct.xyz",
            NetworkMode::Mainnet,
        );
        Ok(Self {
            client,
            sample_size: DEFAULT_SAMPLE_SIZE,
            prove_usd,
        })
    }

    /// Fetch recent fulfilled requests and compute median price per cycle.
    ///
    /// Returns (price_per_cycle_prove, sample_count) where price is in PROVE (f64, full units).
    async fn fetch_market_price(&self) -> Result<(f64, usize), SuccinctPricingError> {
        let response = self
            .client
            .get_filtered_proof_requests(
                None,                                      // version
                Some(FulfillmentStatus::Fulfilled as i32), // fulfilled only
                None,                                      // execution_status
                None,                                      // minimum_deadline
                None,                                      // vk_hash
                None,                                      // requester
                None,                                      // fulfiller
                None,                                      // from
                None,                                      // to
                Some(self.sample_size),                    // limit
                None,                                      // page
                None,                                      // mode (any)
                None,                                      // not_bid_by
                None,                                      // execute_fail_cause
                None,                                      // settlement_status
                None,                                      // error
            )
            .await
            .map_err(|e| SuccinctPricingError::Grpc(e.to_string()))?;

        // Extract (cost_prove, cycles) from either Auction or Base response.
        // Cost is computed from gas_used * gas_price (PROVE wei), falling back to deduction_amount.
        let pairs: Vec<(f64, u64)> = match response {
            GetFilteredProofRequestsResponse::Auction(r) => extract_cost_pairs_auction(r.requests),
            GetFilteredProofRequestsResponse::Base(r) => extract_cost_pairs_base(r.requests),
        };

        let mut prices_per_cycle: Vec<f64> = pairs
            .iter()
            .filter(|(_, cycles)| *cycles > 0)
            .map(|(cost_prove, cycles)| cost_prove / *cycles as f64)
            .collect();

        if prices_per_cycle.is_empty() {
            return Err(SuccinctPricingError::NoData);
        }

        // Median
        prices_per_cycle
            .sort_by(|a: &f64, b: &f64| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = if prices_per_cycle.len() % 2 == 0 {
            let mid = prices_per_cycle.len() / 2;
            (prices_per_cycle[mid - 1] + prices_per_cycle[mid]) / 2.0
        } else {
            prices_per_cycle[prices_per_cycle.len() / 2]
        };

        Ok((median, prices_per_cycle.len()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SuccinctPricingError {
    #[error("gRPC call failed: {0}")]
    Grpc(String),
    #[error("no fulfilled proof requests found")]
    NoData,
    #[error("configuration error: {0}")]
    Config(String),
    #[error("backend {0:?} is not supported by Succinct pricing")]
    UnsupportedBackend(ZkvmBackend),
}

#[async_trait]
impl PricingProvider for SuccinctPricing {
    type Error = SuccinctPricingError;

    fn name(&self) -> &str {
        "succinct"
    }

    fn supports_backend(&self, backend: ZkvmBackend) -> bool {
        matches!(backend, ZkvmBackend::Sp1)
    }

    async fn price(&self, estimate: &CycleEstimate) -> Result<PricingInfo, Self::Error> {
        if !self.supports_backend(estimate.backend) {
            return Err(SuccinctPricingError::UnsupportedBackend(estimate.backend));
        }

        let (price_per_cycle_prove, sample_count) = self.fetch_market_price().await?;
        let total_prove = price_per_cycle_prove * estimate.cycles as f64;
        let cost_usd = total_prove * self.prove_usd;
        let duration_secs = estimate.cycles / SP1_CYCLES_PER_SEC;

        warn!(
            price_per_cycle = %format!("{:.2e}", price_per_cycle_prove),
            total_prove = %format!("{:.6}", total_prove),
            sample_count,
            "succinct market price from recent fulfilled requests"
        );

        Ok(PricingInfo {
            cost_usd,
            duration_secs,
            source: format!("succinct-market-p50-{sample_count}"),
            native_cost: Some(total_prove),
            native_symbol: Some("PROVE".to_string()),
            token_usd_rate: Some(self.prove_usd),
        })
    }
}

/// Extract (cost_in_prove, cycles) from Auction ProofRequest list.
fn extract_cost_pairs_auction(
    requests: Vec<sp1_sdk::network::proto::auction_types::ProofRequest>,
) -> Vec<(f64, u64)> {
    requests
        .into_iter()
        .filter_map(|req| {
            let cycles = req.cycles?;
            // Prefer gas_used * gas_price; fall back to deduction_amount.
            let cost_wei = if let (Some(gas_used), Some(gas_price)) = (req.gas_used, req.gas_price)
            {
                gas_used as u128 * gas_price as u128
            } else if let Some(ref deduction) = req.deduction_amount {
                deduction.parse::<u128>().ok()?
            } else {
                return None;
            };
            Some((cost_wei as f64 / 1e18, cycles))
        })
        .collect()
}

/// Extract (cost_in_prove, cycles) from Base ProofRequest list.
fn extract_cost_pairs_base(
    requests: Vec<sp1_sdk::network::proto::base_types::ProofRequest>,
) -> Vec<(f64, u64)> {
    requests
        .into_iter()
        .filter_map(|req| {
            let cycles = req.cycles?;
            let cost_wei = if let (Some(gas_used), Some(gas_price)) = (req.gas_used, req.gas_price)
            {
                gas_used as u128 * gas_price as u128
            } else if let Some(ref deduction) = req.deduction_amount {
                deduction.parse::<u128>().ok()?
            } else {
                return None;
            };
            Some((cost_wei as f64 / 1e18, cycles))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_calculation() {
        // Simulate price_per_cycle values
        let mut prices: Vec<f64> = vec![1e-15, 3e-15, 2e-15, 5e-15, 4e-15];
        prices.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        // sorted: [1e-15, 2e-15, 3e-15, 4e-15, 5e-15]
        let median = prices[prices.len() / 2]; // 3e-15
        assert!((median - 3e-15).abs() < 1e-20_f64);
    }

    #[test]
    fn price_per_cycle_from_deduction() {
        // deduction_amount = "500000000000000" (0.0005 PROVE)
        // cycles = 250_000_000
        let deduction: u128 = 500_000_000_000_000;
        let cycles: u64 = 250_000_000;
        let deduction_prove = deduction as f64 / 1e18;
        let price_per_cycle = deduction_prove / cycles as f64;
        let total_prove = price_per_cycle * cycles as f64;

        // deduction_prove = 0.0005
        assert!((deduction_prove - 0.0005).abs() < 1e-10);
        // price_per_cycle = 2e-12
        assert!((price_per_cycle - 2e-12).abs() < 1e-20);
        // total at same cycles = 0.0005
        assert!((total_prove - 0.0005).abs() < 1e-10);

        // USD conversion at $1.50/PROVE
        let prove_usd = 1.5;
        let cost_usd = total_prove * prove_usd;
        assert!((cost_usd - 0.00075).abs() < 1e-10);
    }

    #[tokio::test]
    async fn unsupported_backend_returns_error() {
        // Can't create from_env without NETWORK_PRIVATE_KEY, so test the error path
        let result = SuccinctPricing::from_env(1.5);
        if let Ok(pricing) = result {
            let estimate = CycleEstimate {
                cycles: 1_000_000,
                backend: ZkvmBackend::RiscZero,
            };
            assert!(pricing.price(&estimate).await.is_err());
        }
        // If from_env fails (no key), that's expected in CI
    }

    #[tokio::test]
    #[ignore] // requires NETWORK_PRIVATE_KEY or SP1_PRIVATE_KEY and live network access
    async fn live_succinct_pricing() {
        let pricing = SuccinctPricing::from_env(1.5)
            .expect("NETWORK_PRIVATE_KEY or SP1_PRIVATE_KEY required");
        let estimate = CycleEstimate {
            cycles: 250_000_000,
            backend: ZkvmBackend::Sp1,
        };
        let info = pricing.price(&estimate).await.unwrap();
        println!("Live price: ${:.6}", info.cost_usd);
        println!("Native: {:?} {:?}", info.native_cost, info.native_symbol);
        println!("Source: {}", info.source);
        assert!(info.cost_usd > 0.0);
        assert!(info.native_cost.unwrap() > 0.0);
    }
}
