use tracing::warn;

const COINGECKO_ETH_URL: &str =
    "https://api.coingecko.com/api/v3/simple/price?ids=ethereum&vs_currencies=usd";
const COINGECKO_PROVE_URL: &str =
    "https://api.coingecko.com/api/v3/simple/price?ids=succinct&vs_currencies=usd";
const DEFAULT_ETH_USD: f64 = 2000.0;
const DEFAULT_PROVE_USD: f64 = 0.10;
const FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// Fetch the current ETH/USD price.
///
/// Priority: `override_price` > CoinGecko API > `DEFAULT_ETH_USD` fallback.
pub async fn fetch_eth_usd(override_price: Option<f64>) -> f64 {
    if let Some(price) = override_price {
        return price;
    }

    match fetch_token_price(COINGECKO_ETH_URL, "ethereum").await {
        Ok(price) => price,
        Err(e) => {
            warn!("failed to fetch ETH/USD from CoinGecko, using default ${DEFAULT_ETH_USD}: {e}");
            DEFAULT_ETH_USD
        }
    }
}

/// Fetch the current PROVE/USD price.
///
/// Priority: `override_price` > CoinGecko API > `DEFAULT_PROVE_USD` fallback.
pub async fn fetch_prove_usd(override_price: Option<f64>) -> f64 {
    if let Some(price) = override_price {
        return price;
    }

    match fetch_token_price(COINGECKO_PROVE_URL, "succinct").await {
        Ok(price) => price,
        Err(e) => {
            warn!(
                "failed to fetch PROVE/USD from CoinGecko, using default ${DEFAULT_PROVE_USD}: {e}"
            );
            DEFAULT_PROVE_USD
        }
    }
}

async fn fetch_token_price(url: &str, token_key: &str) -> anyhow::Result<f64> {
    let client = reqwest::Client::builder().timeout(FETCH_TIMEOUT).build()?;
    let resp: serde_json::Value = client.get(url).send().await?.json().await?;
    let token = resp
        .get(token_key)
        .ok_or_else(|| anyhow::anyhow!("missing '{token_key}' in CoinGecko response"))?;
    let price = token
        .get("usd")
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| anyhow::anyhow!("missing '{token_key}.usd' in CoinGecko response"))?;
    Ok(price)
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
