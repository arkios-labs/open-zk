use async_trait::async_trait;
use open_zk_core::traits::PricingProvider;
use open_zk_core::types::{CycleEstimate, PricingInfo, ZkvmBackend};
use serde::Deserialize;

const DEFAULT_INDEXER_URL: &str = "https://d2mdvlnmyov1e1.cloudfront.net";
const RISC_ZERO_CYCLES_PER_SEC: u64 = 500_000;
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Price percentile to use from the Boundless Indexer aggregates.
#[derive(Debug, Clone, Copy, Default)]
pub enum Percentile {
    P5,
    P10,
    P25,
    #[default]
    P50,
    P75,
    P90,
    P95,
    P99,
}

impl Percentile {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "p5" => Some(Self::P5),
            "p10" => Some(Self::P10),
            "p25" => Some(Self::P25),
            "p50" => Some(Self::P50),
            "p75" => Some(Self::P75),
            "p90" => Some(Self::P90),
            "p95" => Some(Self::P95),
            "p99" => Some(Self::P99),
            _ => None,
        }
    }

    fn field_name(&self) -> &'static str {
        match self {
            Self::P5 => "p5",
            Self::P10 => "p10",
            Self::P25 => "p25",
            Self::P50 => "p50",
            Self::P75 => "p75",
            Self::P90 => "p90",
            Self::P95 => "p95",
            Self::P99 => "p99",
        }
    }
}

/// Market-based pricing using the Boundless Indexer REST API.
///
/// Queries live `lock_price_per_cycle` data (in ETH wei) and converts
/// to USD using the provided ETH/USD rate.
pub struct BoundlessPricing {
    client: reqwest::Client,
    indexer_url: String,
    percentile: Percentile,
    eth_usd: f64,
}

impl BoundlessPricing {
    pub fn new(eth_usd: f64) -> Self {
        Self::with_options(DEFAULT_INDEXER_URL, Percentile::default(), eth_usd)
    }

    pub fn with_options(indexer_url: &str, percentile: Percentile, eth_usd: f64) -> Self {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent("open-zk/0.1")
            .build()
            .expect("failed to build HTTP client");
        Self {
            client,
            indexer_url: indexer_url.trim_end_matches('/').to_string(),
            percentile,
            eth_usd,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BoundlessPricingError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("no aggregate data returned from indexer")]
    NoData,
    #[error("failed to parse wei value '{0}': {1}")]
    ParseWei(String, String),
    #[error("backend {0:?} is not supported by Boundless pricing")]
    UnsupportedBackend(ZkvmBackend),
}

#[async_trait]
impl PricingProvider for BoundlessPricing {
    type Error = BoundlessPricingError;

    fn name(&self) -> &str {
        "boundless"
    }

    fn supports_backend(&self, backend: ZkvmBackend) -> bool {
        matches!(backend, ZkvmBackend::RiscZero)
    }

    async fn price(&self, estimate: &CycleEstimate) -> Result<PricingInfo, Self::Error> {
        if !self.supports_backend(estimate.backend) {
            return Err(BoundlessPricingError::UnsupportedBackend(estimate.backend));
        }

        let wei_per_cycle = self.fetch_price_per_cycle().await?;
        let total_wei = wei_per_cycle as f64 * estimate.cycles as f64;
        let total_eth = total_wei / 1e18;
        let cost_usd = total_eth * self.eth_usd;
        let duration_secs = estimate.cycles / RISC_ZERO_CYCLES_PER_SEC;
        let source = format!("boundless-market-{}", self.percentile.field_name());

        Ok(PricingInfo {
            cost_usd,
            duration_secs,
            source,
            native_cost: Some(total_eth),
            native_symbol: Some("ETH".to_string()),
            token_usd_rate: Some(self.eth_usd),
        })
    }
}

impl BoundlessPricing {
    async fn fetch_price_per_cycle(&self) -> Result<u128, BoundlessPricingError> {
        let url = format!(
            "{}/v1/market/aggregates?aggregation=hourly&limit=1&sort=desc",
            self.indexer_url
        );
        let text = self.client.get(&url).send().await?.text().await?;
        let resp: AggregateResponse = serde_json::from_str(&text).map_err(|e| {
            BoundlessPricingError::ParseWei(text.chars().take(200).collect(), e.to_string())
        })?;
        let entry = resp.data.first().ok_or(BoundlessPricingError::NoData)?;
        entry.price_for_percentile(&self.percentile)
    }
}

// --- Indexer API response types ---

#[derive(Debug, Deserialize)]
struct AggregateResponse {
    data: Vec<AggregateEntry>,
}

#[derive(Debug, Deserialize)]
struct AggregateEntry {
    p5_lock_price_per_cycle: String,
    p10_lock_price_per_cycle: String,
    p25_lock_price_per_cycle: String,
    p50_lock_price_per_cycle: String,
    p75_lock_price_per_cycle: String,
    p90_lock_price_per_cycle: String,
    p95_lock_price_per_cycle: String,
    p99_lock_price_per_cycle: String,
}

impl AggregateEntry {
    fn price_for_percentile(&self, p: &Percentile) -> Result<u128, BoundlessPricingError> {
        let s = match p {
            Percentile::P5 => &self.p5_lock_price_per_cycle,
            Percentile::P10 => &self.p10_lock_price_per_cycle,
            Percentile::P25 => &self.p25_lock_price_per_cycle,
            Percentile::P50 => &self.p50_lock_price_per_cycle,
            Percentile::P75 => &self.p75_lock_price_per_cycle,
            Percentile::P90 => &self.p90_lock_price_per_cycle,
            Percentile::P95 => &self.p95_lock_price_per_cycle,
            Percentile::P99 => &self.p99_lock_price_per_cycle,
        };
        s.parse::<u128>()
            .map_err(|e| BoundlessPricingError::ParseWei(s.clone(), e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RESPONSE: &str = r#"{
        "data": [{
            "p5_lock_price_per_cycle": "10000",
            "p10_lock_price_per_cycle": "20000",
            "p25_lock_price_per_cycle": "35000",
            "p50_lock_price_per_cycle": "46290",
            "p75_lock_price_per_cycle": "60000",
            "p90_lock_price_per_cycle": "80000",
            "p95_lock_price_per_cycle": "100000",
            "p99_lock_price_per_cycle": "150000"
        }]
    }"#;

    #[test]
    fn parse_aggregate_response() {
        let resp: AggregateResponse = serde_json::from_str(SAMPLE_RESPONSE).unwrap();
        assert_eq!(resp.data.len(), 1);
        let entry = &resp.data[0];
        assert_eq!(entry.price_for_percentile(&Percentile::P50).unwrap(), 46290);
        assert_eq!(entry.price_for_percentile(&Percentile::P5).unwrap(), 10000);
    }

    #[test]
    fn price_calculation() {
        // 250M cycles at p50 = 46290 wei/cycle
        let wei_per_cycle: u128 = 46290;
        let cycles: u64 = 250_000_000;
        let total_wei = wei_per_cycle as f64 * cycles as f64;
        let total_eth = total_wei / 1e18;
        let eth_usd = 3500.0;
        let cost_usd = total_eth * eth_usd;

        // total_eth = 46290 * 250_000_000 / 1e18 = 0.0000115725
        assert!((total_eth - 0.0000115725).abs() < 1e-10);
        // cost_usd = 0.0000115725 * 3500 = 0.0405
        assert!((cost_usd - 0.04050375).abs() < 1e-6);
    }

    #[tokio::test]
    async fn unsupported_backend_returns_error() {
        let pricing = BoundlessPricing::new(3500.0);
        let estimate = CycleEstimate {
            cycles: 1_000_000,
            backend: ZkvmBackend::Sp1,
        };
        let result = pricing.price(&estimate).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore] // requires live network access
    async fn live_boundless_indexer() {
        let pricing = BoundlessPricing::new(3500.0);
        let estimate = CycleEstimate {
            cycles: 250_000_000,
            backend: ZkvmBackend::RiscZero,
        };
        let info = pricing.price(&estimate).await.unwrap();
        println!("Live price: ${:.6}", info.cost_usd);
        println!("Native: {:?} {:?}", info.native_cost, info.native_symbol);
        println!("Source: {}", info.source);
        assert!(info.cost_usd > 0.0);
        assert!(info.native_cost.unwrap() > 0.0);
    }
}
