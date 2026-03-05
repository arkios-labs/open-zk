//! E2E test: mock proof → on-chain submission → state verification.
//!
//! Prerequisites:
//!   - Running OP Stack devnet (`just devnet-up`)
//!   - OpenZkL2OutputOracle deployed to devnet
//!
//! Run:
//!   ORACLE_ADDRESS=0x76ca03a67C049477FfB09694dFeF00416dB69746 \
//!   cargo test -p open-zk-contracts --features rpc \
//!     --test onchain_e2e -- --ignored --nocapture

#![cfg(feature = "rpc")]

use alloy_primitives::{Address, B256};
use alloy_provider::ProviderBuilder;
use open_zk_contracts::abi::IOpenZkL2OutputOracle;
use open_zk_contracts::{ProofSubmitter, RpcProofSubmitter};
use open_zk_core::types::{ProofArtifact, ProvingMode, StateTransitionJournal, ZkvmBackend};

const L1_RPC: &str = "http://127.0.0.1:8545";
const DEPLOYER_KEY: &str = "0x4bbbf85ce3377467afe5d46f804f221813b2bb87f24d81f60f1fcdbf7cbf4356";

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
}

fn oracle_address() -> Address {
    std::env::var("ORACLE_ADDRESS")
        .expect("ORACLE_ADDRESS env var required")
        .parse()
        .expect("invalid ORACLE_ADDRESS")
}

async fn read_latest_block_number(oracle_addr: Address) -> u64 {
    let url: url::Url = L1_RPC.parse().unwrap();
    let provider = ProviderBuilder::new().connect_http(url);
    let oracle = IOpenZkL2OutputOracle::new(oracle_addr, &provider);
    oracle.latestBlockNumber().call().await.unwrap()
}

async fn read_latest_output_root(oracle_addr: Address) -> B256 {
    let url: url::Url = L1_RPC.parse().unwrap();
    let provider = ProviderBuilder::new().connect_http(url);
    let oracle = IOpenZkL2OutputOracle::new(oracle_addr, &provider);
    oracle.latestOutputRoot().call().await.unwrap()
}

async fn read_is_block_proven(oracle_addr: Address, block_number: u64) -> bool {
    let url: url::Url = L1_RPC.parse().unwrap();
    let provider = ProviderBuilder::new().connect_http(url);
    let oracle = IOpenZkL2OutputOracle::new(oracle_addr, &provider);
    oracle.isBlockProven(block_number).call().await.unwrap()
}

/// Full E2E: build journal → submit SP1 proof to on-chain contract → verify state.
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn test_onchain_sp1_proof_submission() {
    init_tracing();

    let oracle_addr = oracle_address();
    println!("Oracle address: {oracle_addr}");

    let journal = StateTransitionJournal {
        l1_head: B256::repeat_byte(0x11),
        l2_pre_root: B256::repeat_byte(0x22),
        l2_post_root: B256::repeat_byte(0x33),
        l2_block_number: 10,
        rollup_config_hash: B256::repeat_byte(0x44),
        program_id: B256::repeat_byte(0x55),
    };

    let proof = ProofArtifact {
        backend: ZkvmBackend::Sp1,
        mode: ProvingMode::Groth16,
        proof_bytes: vec![0xDE, 0xAD, 0xBE, 0xEF],
        public_values: journal.to_abi_bytes(),
        program_id: B256::ZERO,
        cycle_count: Some(100_000_000),
    };

    let submitter = RpcProofSubmitter::new(
        L1_RPC.to_string(),
        oracle_addr,
        DEPLOYER_KEY.to_string(),
    );

    let tx_hash = submitter
        .submit_proof(&journal, &proof)
        .await
        .expect("proof submission failed");

    println!("Proof submitted! tx_hash: {tx_hash}");
    assert_ne!(tx_hash, B256::ZERO);

    // Verify on-chain state
    let latest_block = read_latest_block_number(oracle_addr).await;
    println!("On-chain latestBlockNumber: {latest_block}");
    assert_eq!(latest_block, 10);

    let latest_root = read_latest_output_root(oracle_addr).await;
    println!("On-chain latestOutputRoot: {latest_root}");
    assert_eq!(latest_root, B256::repeat_byte(0x33));

    assert!(read_is_block_proven(oracle_addr, 10).await);

    println!("On-chain SP1 E2E PASSED!");
}

/// Full E2E: RISC Zero mock proof submission.
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn test_onchain_risczero_proof_submission() {
    init_tracing();

    let oracle_addr = oracle_address();

    let journal = StateTransitionJournal {
        l1_head: B256::repeat_byte(0xAA),
        l2_pre_root: B256::repeat_byte(0xBB),
        l2_post_root: B256::repeat_byte(0xCC),
        l2_block_number: 20,
        rollup_config_hash: B256::repeat_byte(0xDD),
        program_id: B256::repeat_byte(0xEE),
    };

    let proof = ProofArtifact {
        backend: ZkvmBackend::RiscZero,
        mode: ProvingMode::Groth16,
        proof_bytes: vec![0xCA, 0xFE, 0xBA, 0xBE],
        public_values: journal.to_abi_bytes(),
        program_id: B256::ZERO,
        cycle_count: Some(200_000_000),
    };

    let submitter = RpcProofSubmitter::new(
        L1_RPC.to_string(),
        oracle_addr,
        DEPLOYER_KEY.to_string(),
    );

    let tx_hash = submitter
        .submit_proof(&journal, &proof)
        .await
        .expect("RISC Zero proof submission failed");

    println!("RISC Zero proof submitted! tx_hash: {tx_hash}");

    let latest_block = read_latest_block_number(oracle_addr).await;
    println!("On-chain latestBlockNumber: {latest_block}");
    assert_eq!(latest_block, 20);

    assert!(read_is_block_proven(oracle_addr, 20).await);

    println!("On-chain RISC Zero E2E PASSED!");
}
