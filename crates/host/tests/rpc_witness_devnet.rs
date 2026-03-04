//! Real witness generation test against a running devnet.
//!
//! Requires:
//!   - Running OP Stack devnet (`just devnet-up`)
//!   - `kona` feature enabled
//!
//! Run with:
//!   cargo test -p open-zk-host --features kona --test rpc_witness_devnet -- --ignored --nocapture

#![cfg(feature = "kona")]

use alloy_primitives::B256;
use open_zk_core::traits::WitnessProvider;
use open_zk_core::types::{ProofRequest, ProvingMode};
use open_zk_host::witness::RpcWitnessProvider;
use std::path::PathBuf;
use std::time::Instant;

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
}

const L1_RPC: &str = "http://127.0.0.1:8545";
const L2_RPC: &str = "http://127.0.0.1:9545";
const L1_BEACON: &str = "http://127.0.0.1:5052";

/// Fetch L1 head hash from devnet.
async fn get_l1_head() -> B256 {
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

/// Fetch L2 output root at a given block from op-node.
async fn get_l2_output_root(block_number: u64) -> B256 {
    use alloy_provider::{Provider, ProviderBuilder};

    let url: url::Url = "http://127.0.0.1:7545".parse().unwrap();
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

/// Test pre-flight RPC calls only (no kona pipeline).
/// Validates that we can fetch L2 block hashes and output roots from devnet.
#[tokio::test]
#[ignore]
async fn test_preflight_rpc_calls() {
    use alloy_provider::{Provider, ProviderBuilder};
    use alloy_rpc_types_eth::BlockNumberOrTag;

    let l1_head = get_l1_head().await;
    let l2_start_output_root = get_l2_output_root(1).await;
    println!("L1 head: {l1_head}");
    println!("L2 start output root (block 1): {l2_start_output_root}");

    // Test what our RpcWitnessProvider does in pre-flight:
    // 1. Fetch start block hash
    let l2_url: url::Url = L2_RPC.parse().unwrap();
    let l2_provider = ProviderBuilder::new().connect_http(l2_url);

    let start_block = l2_provider
        .get_block_by_number(BlockNumberOrTag::Number(1))
        .await
        .unwrap()
        .unwrap();
    println!("L2 block 1 hash: {}", start_block.header.hash);

    // 2. Fetch end block output root (fallback to block header derivation)
    let end_block = l2_provider
        .get_block_by_number(BlockNumberOrTag::Number(2))
        .await
        .unwrap()
        .unwrap();
    println!("L2 block 2 hash: {}", end_block.header.hash);
    println!("L2 block 2 state_root: {}", end_block.header.state_root);

    // 3. Compute output root from block header (our fallback)
    use alloy_primitives::keccak256;
    let mut payload = [0u8; 128];
    payload[32..64].copy_from_slice(end_block.header.state_root.as_slice());
    let wr = end_block.header.withdrawals_root.unwrap_or(B256::ZERO);
    payload[64..96].copy_from_slice(wr.as_slice());
    payload[96..128].copy_from_slice(end_block.header.hash.as_slice());
    let derived_output_root = keccak256(payload);
    println!("Derived output root (block 2): {derived_output_root}");

    // Compare with op-node's value
    let op_node_root = get_l2_output_root(2).await;
    println!("Op-node output root (block 2): {op_node_root}");

    // These may differ because the OP Stack output root computation includes
    // the message passer storage root, not the withdrawals root
    println!("Match: {}", derived_output_root == op_node_root);
}

/// Test real witness generation for a single L2 block (block 1→2).
///
/// This tests the full kona pipeline:
/// 1. Pre-flight RPC calls (fetch L2 block hash + output root)
/// 2. SingleChainHost configuration
/// 3. kona_client::single::run (native derivation)
/// 4. Preimage collection and serialization
#[tokio::test]
#[ignore]
async fn test_real_witness_generation_single_block() {
    init_tracing();
    let l1_head = get_l1_head().await;
    let l2_start_output_root = get_l2_output_root(1).await;

    println!("L1 head: {l1_head}");
    println!("L2 output root at block 1: {l2_start_output_root}");

    let provider = RpcWitnessProvider::new(
        L1_RPC.to_string(),
        L2_RPC.to_string(),
        L1_BEACON.to_string(),
    )
    .with_rollup_config(PathBuf::from("../../devnet-rollup.json"))
    .with_chain_id(901);

    let request = ProofRequest {
        l1_head,
        l2_start_block: 1,
        l2_end_block: 2,
        l2_start_output_root,
        mode: ProvingMode::Execute,
    };

    println!("Starting witness generation for blocks 1→2...");
    let start = Instant::now();

    match provider.generate_witness(&request).await {
        Ok(witness) => {
            let elapsed = start.elapsed();
            println!("Witness generated in {:.2}s", elapsed.as_secs_f64());
            println!("  boot_info:  {} bytes", witness.boot_info.len());
            println!("  oracle_data: {} bytes", witness.oracle_data.len());
            println!("  blob_data:   {} bytes", witness.blob_data.len());
            assert!(!witness.boot_info.is_empty(), "boot_info should not be empty");
            assert!(!witness.oracle_data.is_empty(), "oracle_data should not be empty");
        }
        Err(e) => {
            let elapsed = start.elapsed();
            println!("Witness generation failed after {:.2}s: {e}", elapsed.as_secs_f64());
            println!("Error details: {e:?}");
            panic!("Witness generation failed: {e}");
        }
    }
}
