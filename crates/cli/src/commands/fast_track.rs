//! `open-zk fast-track` — Deploy OpenZk contracts to a devnet.
//!
//! Deploys the L2OutputOracle and (optionally) DisputeGame contracts via
//! `forge script`, configuring them for the running devnet.

use alloy_primitives::Address;
use clap::Args;
use std::process::Command;
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

    // Validate deployer key
    let deployer_key = args
        .deployer_key
        .strip_prefix("0x")
        .unwrap_or(&args.deployer_key);
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
    println!("Mock mode:  {mock_mode}");
    println!();

    // Step 1: Verify RPC connectivity
    info!("step 1/4: verifying RPC connectivity");
    verify_connectivity(&args.l1_rpc_url, &args.l2_rpc_url).await?;

    // Step 2: Verify Foundry is installed
    info!("step 2/4: checking Foundry installation");
    check_foundry()?;

    // Step 3: Deploy contracts via forge script
    info!("step 3/4: deploying contracts via forge script");
    let output = deploy_via_forge(&args)?;

    // Step 4: Parse deployed addresses from forge output
    info!("step 4/4: parsing deployment results");
    let result = parse_deployment_output(&output, &args)?;

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

    // Write deployment info to JSON
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

/// Check that `forge` is available on PATH.
fn check_foundry() -> anyhow::Result<()> {
    let output = Command::new("forge").arg("--version").output();
    match output {
        Ok(o) if o.status.success() => {
            let version = String::from_utf8_lossy(&o.stdout);
            println!("Foundry: {}", version.trim());
            Ok(())
        }
        _ => anyhow::bail!(
            "Foundry not found. Install it with: curl -L https://foundry.paradigm.xyz | bash"
        ),
    }
}

/// Deploy contracts by running `forge script`.
fn deploy_via_forge(args: &FastTrackArgs) -> anyhow::Result<String> {
    let deployer_key_prefixed = if args.deployer_key.starts_with("0x") {
        args.deployer_key.clone()
    } else {
        format!("0x{}", args.deployer_key)
    };

    let mut cmd = Command::new("forge");
    cmd.current_dir("contracts")
        .arg("script")
        .arg("script/DeployDevnet.s.sol")
        .arg("--rpc-url")
        .arg(&args.l1_rpc_url)
        .arg("--broadcast")
        .env("DEPLOYER_PRIVATE_KEY", &deployer_key_prefixed)
        .env(
            "DEPLOY_DISPUTE_GAME",
            if args.with_dispute_game {
                "true"
            } else {
                "false"
            },
        )
        .env("CHALLENGE_TIMEOUT", args.challenge_timeout.to_string());

    println!("Running forge script...");
    let output = cmd
        .output()
        .map_err(|e| anyhow::anyhow!("failed to run forge: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        anyhow::bail!("forge script failed:\nstdout:\n{stdout}\nstderr:\n{stderr}");
    }

    // Print forge output for transparency
    for line in stdout.lines() {
        if line.contains("::") || line.contains("Deployed") || line.contains("0x") {
            println!("  {line}");
        }
    }

    Ok(stdout.to_string())
}

/// Parse contract addresses from forge script output.
fn parse_deployment_output(output: &str, args: &FastTrackArgs) -> anyhow::Result<DeploymentResult> {
    let oracle_address = extract_address(output, "OpenZkL2OutputOracle").ok_or_else(|| {
        anyhow::anyhow!("failed to find OpenZkL2OutputOracle address in forge output")
    })?;

    let dispute_game_address = if args.with_dispute_game {
        Some(extract_address(output, "OpenZkDisputeGame").ok_or_else(|| {
            anyhow::anyhow!("failed to find OpenZkDisputeGame address in forge output")
        })?)
    } else {
        None
    };

    Ok(DeploymentResult {
        oracle_address,
        dispute_game_address,
        _deployer: Address::ZERO,
        starting_block: args.starting_block,
    })
}

/// Extract an address from forge script output lines like:
/// `OpenZkL2OutputOracle:     0x1234...`
fn extract_address(output: &str, contract_name: &str) -> Option<Address> {
    for line in output.lines() {
        if line.contains(contract_name) {
            // Find the 0x address in the line
            if let Some(pos) = line.find("0x") {
                let addr_str = &line[pos..];
                // Take next 42 chars (0x + 40 hex)
                let addr_candidate = if addr_str.len() >= 42 {
                    &addr_str[..42]
                } else {
                    addr_str.trim()
                };
                if let Ok(addr) = addr_candidate.parse::<Address>() {
                    return Some(addr);
                }
            }
        }
    }
    None
}
