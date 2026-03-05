//! Shared helpers for devnet integration tests.

use alloy_primitives::B256;

pub const L1_RPC: &str = "http://127.0.0.1:8545";
pub const L2_RPC: &str = "http://127.0.0.1:9545";
pub const L1_BEACON: &str = "http://127.0.0.1:5052";
pub const OP_NODE: &str = "http://127.0.0.1:7545";

pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
}

pub async fn get_l1_head() -> B256 {
    use alloy_provider::{Provider, ProviderBuilder};
    use alloy_rpc_types_eth::BlockNumberOrTag;

    let url: url::Url = L1_RPC.parse().unwrap();
    let provider = ProviderBuilder::new().connect_http(url);
    let block = provider
        .get_block_by_number(BlockNumberOrTag::Latest)
        .await
        .unwrap()
        .unwrap();
    block.header.hash
}

pub async fn get_l2_output_root(block_number: u64) -> B256 {
    use alloy_provider::{Provider, ProviderBuilder};

    let url: url::Url = OP_NODE.parse().unwrap();
    let provider = ProviderBuilder::new().connect_http(url);
    let resp: serde_json::Value = provider
        .raw_request(
            "optimism_outputAtBlock".into(),
            [format!("0x{:x}", block_number)],
        )
        .await
        .unwrap();
    resp["outputRoot"]
        .as_str()
        .unwrap()
        .parse::<B256>()
        .unwrap()
}
