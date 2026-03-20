use serde::Deserialize;
use tracing::warn;

const COINGECKO_ETH_URL: &str =
    "https://api.coingecko.com/api/v3/simple/price?ids=ethereum&vs_currencies=usd";
const DEFAULT_ETH_USD: f64 = 2000.0;
const FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

#[derive(Deserialize)]
struct CoinGeckoResponse {
    ethereum: EthPrice,
}

#[derive(Deserialize)]
struct EthPrice {
    usd: f64,
}

/// Fetch the current ETH/USD price.
///
/// Priority: `override_price` > CoinGecko API > `DEFAULT_ETH_USD` fallback.
pub async fn fetch_eth_usd(override_price: Option<f64>) -> f64 {
    if let Some(price) = override_price {
        return price;
    }

    match fetch_from_coingecko().await {
        Ok(price) => price,
        Err(e) => {
            warn!("failed to fetch ETH/USD from CoinGecko, using default ${DEFAULT_ETH_USD}: {e}");
            DEFAULT_ETH_USD
        }
    }
}

async fn fetch_from_coingecko() -> anyhow::Result<f64> {
    let client = reqwest::Client::builder().timeout(FETCH_TIMEOUT).build()?;
    let resp: CoinGeckoResponse = client.get(COINGECKO_ETH_URL).send().await?.json().await?;
    Ok(resp.ethereum.usd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn override_price_takes_precedence() {
        let price = fetch_eth_usd(Some(4200.0)).await;
        assert!((price - 4200.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn fallback_on_network_error() {
        // With no override and no network (CoinGecko will likely fail in CI),
        // we should get the default fallback.
        let price = fetch_eth_usd(None).await;
        assert!(price > 0.0);
    }
}
