//! Real witness generation test against a running devnet.
//!
//! Requires:
//!   - Running OP Stack devnet (`just devnet-up`)
//!   - `kona` feature enabled
//!
//! Run with:
//!   cargo test -p open-zk-host --features kona --test rpc_witness_devnet -- --ignored --nocapture

#![cfg(feature = "kona")]

mod common;

use alloy_primitives::B256;
use common::{get_l1_head, get_l2_output_root, init_tracing, L1_BEACON, L1_RPC, L2_RPC, OP_NODE};
use open_zk_core::traits::WitnessProvider;
use open_zk_core::types::{ProofRequest, ProvingMode};
use open_zk_host::witness::RpcWitnessProvider;
use std::time::Instant;

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

    let l2_url: url::Url = L2_RPC.parse().unwrap();
    let l2_provider = ProviderBuilder::new().connect_http(l2_url);

    let start_block = l2_provider
        .get_block_by_number(BlockNumberOrTag::Number(1))
        .await
        .unwrap()
        .unwrap();
    println!("L2 block 1 hash: {}", start_block.header.hash);

    let end_block = l2_provider
        .get_block_by_number(BlockNumberOrTag::Number(2))
        .await
        .unwrap()
        .unwrap();
    println!("L2 block 2 hash: {}", end_block.header.hash);
    println!("L2 block 2 state_root: {}", end_block.header.state_root);

    use alloy_primitives::keccak256;
    let mut payload = [0u8; 128];
    payload[32..64].copy_from_slice(end_block.header.state_root.as_slice());
    let wr = end_block.header.withdrawals_root.unwrap_or(B256::ZERO);
    payload[64..96].copy_from_slice(wr.as_slice());
    payload[96..128].copy_from_slice(end_block.header.hash.as_slice());
    let derived_output_root = keccak256(payload);
    println!("Derived output root (block 2): {derived_output_root}");

    let op_node_root = get_l2_output_root(2).await;
    println!("Op-node output root (block 2): {op_node_root}");

    println!("Match: {}", derived_output_root == op_node_root);
}

/// Test rollup config fetching from OP Node.
#[tokio::test]
#[ignore]
async fn test_rollup_config_fetch_from_op_node() {
    init_tracing();

    use alloy_provider::{Provider, ProviderBuilder};

    let op_url: url::Url = OP_NODE.parse().unwrap();
    let op_provider = ProviderBuilder::new().connect_http(op_url);

    let config: serde_json::Value = op_provider
        .raw_request::<_, serde_json::Value>("optimism_rollupConfig".into(), ())
        .await
        .unwrap();

    println!("Rollup config keys: {:?}", config.as_object().unwrap().keys().collect::<Vec<_>>());
    assert!(config.get("l2_chain_id").is_some() || config.get("chain_id").is_some(),
        "rollup config should contain chain ID");
    println!("Rollup config fetched successfully from OP Node");
}

/// Test real witness generation for a single L2 block (block 1→2).
#[tokio::test(flavor = "multi_thread")]
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
    .with_op_node_url(OP_NODE.to_string())
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
