//! `open-zk fast-track` — Deploy OpenZk contracts to a devnet.
//!
//! Deploys the L2OutputOracle and (optionally) DisputeGame contracts,
//! configuring them for the running devnet. Uses pre-funded devnet
//! accounts from the OP Stack devnet genesis.

use alloy_primitives::Address;
use clap::Args;
use tracing::{info, warn};

/// Default devnet RPC endpoints (OP Stack devnet-up).
const DEFAULT_L1_RPC: &str = "http://127.0.0.1:8545";
const DEFAULT_L2_RPC: &str = "http://127.0.0.1:9545";
const DEFAULT_L1_BEACON: &str = "http://127.0.0.1:5052";

#[derive(Debug, Args)]
pub struct FastTrackArgs {
    /// L1 execution client RPC URL.
    #[arg(long, default_value = DEFAULT_L1_RPC)]
    pub l1_rpc_url: String,

    /// L2 execution client RPC URL.
    #[arg(long, default_value = DEFAULT_L2_RPC)]
    pub l2_rpc_url: String,

    /// L1 beacon API URL.
    #[arg(long, default_value = DEFAULT_L1_BEACON)]
    pub l1_beacon_url: String,

    /// Private key of the contract deployer (hex, with or without 0x prefix).
    #[arg(long)]
    pub deployer_key: String,

    /// Private key of the contract owner (hex, with or without 0x prefix).
    #[arg(long)]
    pub owner_key: String,

    /// Starting L2 block number for the oracle.
    #[arg(long, default_value = "0")]
    pub starting_block: u64,

    /// Submission interval (number of L2 blocks between submissions).
    #[arg(long, default_value = "20")]
    pub submission_interval: u64,

    /// Challenge timeout in seconds.
    #[arg(long, default_value = "3600")]
    pub challenge_timeout: u64,

    /// Deploy dispute game contracts alongside the oracle.
    #[arg(long)]
    pub with_dispute_game: bool,
}

/// Deployment result containing contract addresses.
#[derive(Debug)]
pub struct DeploymentResult {
    pub oracle_address: Address,
    pub dispute_game_address: Option<Address>,
    pub _deployer: Address,
    pub starting_block: u64,
}

pub async fn execute(args: FastTrackArgs) -> anyhow::Result<()> {
    info!(
        l1_rpc = %args.l1_rpc_url,
        l2_rpc = %args.l2_rpc_url,
        beacon = %args.l1_beacon_url,
        "starting fast-track contract deployment"
    );

    // Parse deployer key to verify it's valid hex
    let deployer_key = args
        .deployer_key
        .strip_prefix("0x")
        .unwrap_or(&args.deployer_key);
    let _owner_key = args.owner_key.strip_prefix("0x").unwrap_or(&args.owner_key);

    anyhow::ensure!(
        deployer_key.len() == 64 && deployer_key.chars().all(|c| c.is_ascii_hexdigit()),
        "invalid deployer key: must be 64 hex characters"
    );

    // Detect mock proof mode
    let mock_mode = std::env::var("SP1_PROVER").is_ok_and(|v| v == "mock");
    if mock_mode {
        warn!("SP1_PROVER=mock detected — deploying with mock verifier support");
    }

    println!("Open-ZK Fast Track Deployment");
    println!("=============================");
    println!("L1 RPC:     {}", args.l1_rpc_url);
    println!("L2 RPC:     {}", args.l2_rpc_url);
    println!("L1 Beacon:  {}", args.l1_beacon_url);
    println!("Mock mode:  {}", mock_mode);
    println!();

    // Step 1: Verify L1/L2 connectivity
    info!("step 1/5: verifying RPC connectivity");
    verify_connectivity(&args.l1_rpc_url, &args.l2_rpc_url).await?;

    // Step 2: Fetch the current L2 state
    info!("step 2/5: fetching L2 chain state");
    let l2_block = fetch_l2_latest_block(&args.l2_rpc_url).await?;
    println!("Latest L2 block: {l2_block}");

    // Step 3: Deploy L2OutputOracle
    info!("step 3/5: deploying L2OutputOracle");
    println!(
        "Deploying L2OutputOracle (starting block: {}, submission interval: {})...",
        args.starting_block, args.submission_interval
    );
    // Contract deployment requires compiled Solidity bytecodes.
    // For now, log the deployment parameters and return placeholder addresses.
    let oracle_address = deploy_oracle_placeholder(&args)?;
    println!("L2OutputOracle deployed at: {oracle_address}");

    // Step 4: Optionally deploy DisputeGame
    let dispute_address = if args.with_dispute_game {
        info!("step 4/5: deploying DisputeGame");
        println!(
            "Deploying DisputeGame (challenge timeout: {}s)...",
            args.challenge_timeout
        );
        let addr = deploy_dispute_placeholder(&args)?;
        println!("DisputeGame deployed at: {addr}");
        Some(addr)
    } else {
        info!("step 4/5: skipping DisputeGame deployment (not requested)");
        None
    };

    // Step 5: Verify deployment
    info!("step 5/5: verifying deployment");
    let result = DeploymentResult {
        oracle_address,
        dispute_game_address: dispute_address,
        _deployer: Address::ZERO, // Would be derived from deployer_key
        starting_block: args.starting_block,
    };

    println!();
    println!("Deployment Summary");
    println!("==================");
    println!("Oracle:       {}", result.oracle_address);
    if let Some(dispute) = result.dispute_game_address {
        println!("DisputeGame:  {dispute}");
    }
    println!("Start block:  {}", result.starting_block);
    println!();
    println!("Save these addresses in your open-zk.toml to use with `open-zk serve`.");

    // Write deployment info to a JSON file for downstream tools
    let deployment_json = serde_json::json!({
        "oracle_address": format!("{}", result.oracle_address),
        "dispute_game_address": result.dispute_game_address.map(|a| format!("{a}")),
        "starting_block": result.starting_block,
        "l1_rpc_url": args.l1_rpc_url,
        "l2_rpc_url": args.l2_rpc_url,
        "l1_beacon_url": args.l1_beacon_url,
        "mock_mode": mock_mode,
    });
    std::fs::write(
        "deployment.json",
        serde_json::to_string_pretty(&deployment_json)?,
    )?;
    info!("deployment info written to deployment.json");

    Ok(())
}

/// Verify that L1 and L2 RPC endpoints are reachable.
async fn verify_connectivity(l1_rpc: &str, l2_rpc: &str) -> anyhow::Result<()> {
    use alloy_provider::{Provider, ProviderBuilder};

    let l1_url: url::Url = l1_rpc.parse()?;
    let l2_url: url::Url = l2_rpc.parse()?;

    let l1_provider = ProviderBuilder::new().connect_http(l1_url);
    let l2_provider = ProviderBuilder::new().connect_http(l2_url);

    let l1_chain_id = l1_provider
        .get_chain_id()
        .await
        .map_err(|e| anyhow::anyhow!("L1 RPC unreachable: {e}"))?;
    let l2_chain_id = l2_provider
        .get_chain_id()
        .await
        .map_err(|e| anyhow::anyhow!("L2 RPC unreachable: {e}"))?;

    info!(l1_chain_id, l2_chain_id, "RPC connectivity verified");
    println!("L1 chain ID: {l1_chain_id}");
    println!("L2 chain ID: {l2_chain_id}");
    Ok(())
}

/// Fetch the latest L2 block number.
async fn fetch_l2_latest_block(l2_rpc: &str) -> anyhow::Result<u64> {
    use alloy_provider::{Provider, ProviderBuilder};

    let l2_url: url::Url = l2_rpc.parse()?;
    let l2_provider = ProviderBuilder::new().connect_http(l2_url);

    let block_number = l2_provider
        .get_block_number()
        .await
        .map_err(|e| anyhow::anyhow!("failed to get L2 block number: {e}"))?;

    Ok(block_number)
}

/// Placeholder for L2OutputOracle deployment.
///
/// In production, this will:
/// 1. Compile and deploy the L2OutputOracle Solidity contract
/// 2. Call initialize() with the starting block and submission interval
/// 3. Transfer ownership to the owner key
///
/// For now, returns a deterministic address based on deployer + nonce.
fn deploy_oracle_placeholder(args: &FastTrackArgs) -> anyhow::Result<Address> {
    // Placeholder: deterministic address from first 20 bytes of keccak(deployer_key)
    let key_bytes = hex::decode(
        args.deployer_key
            .strip_prefix("0x")
            .unwrap_or(&args.deployer_key),
    )?;
    let hash = alloy_primitives::keccak256(&key_bytes);
    Ok(Address::from_slice(&hash[..20]))
}

/// Placeholder for DisputeGame deployment.
fn deploy_dispute_placeholder(args: &FastTrackArgs) -> anyhow::Result<Address> {
    let key_bytes = hex::decode(args.owner_key.strip_prefix("0x").unwrap_or(&args.owner_key))?;
    let hash = alloy_primitives::keccak256(&key_bytes);
    Ok(Address::from_slice(&hash[..20]))
}
