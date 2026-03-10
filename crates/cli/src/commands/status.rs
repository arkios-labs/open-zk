//! `open-zk status` — Check the current proving status.

use crate::config::CliConfig;
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Path to config file. Defaults to `open-zk.toml`.
    #[arg(long, short, default_value = "open-zk.toml")]
    pub config: PathBuf,

    /// Oracle contract address on L1 (hex, 0x-prefixed).
    #[arg(long)]
    pub oracle_address: Option<String>,
}

pub async fn execute(args: StatusArgs) -> anyhow::Result<()> {
    let config = CliConfig::from_file(&args.config)?;

    println!("open-zk Status");
    println!("==============");
    println!();
    println!("Network:");
    println!("  L1 RPC: {}", config.network.l1_rpc_url);
    println!("  L2 RPC: {}", config.network.l2_rpc_url);
    println!("  Beacon: {}", config.network.l1_beacon_url);
    println!();

    // Fetch live chain state
    fetch_chain_status(&config, args.oracle_address.as_deref()).await?;

    Ok(())
}

async fn fetch_chain_status(
    config: &CliConfig,
    oracle_address: Option<&str>,
) -> anyhow::Result<()> {
    use alloy_provider::{Provider, ProviderBuilder};
    use alloy_rpc_types_eth::BlockNumberOrTag;

    let l1_url: url::Url = config.network.l1_rpc_url.parse()?;
    let l2_url: url::Url = config.network.l2_rpc_url.parse()?;

    let l1_provider = ProviderBuilder::new().connect_http(l1_url);
    let l2_provider = ProviderBuilder::new().connect_http(l2_url);

    // L1 state
    let l1_block = l1_provider
        .get_block_number()
        .await
        .map_err(|e| anyhow::anyhow!("L1 RPC unreachable: {e}"))?;
    println!("L1:");
    println!("  Latest block: {l1_block}");

    // L2 state
    let l2_latest = l2_provider
        .get_block_number()
        .await
        .map_err(|e| anyhow::anyhow!("L2 RPC unreachable: {e}"))?;

    let l2_safe = l2_provider
        .get_block_by_number(BlockNumberOrTag::Safe)
        .await
        .ok()
        .flatten()
        .map(|b| b.header.number);

    println!("L2:");
    println!("  Latest block: {l2_latest}");
    if let Some(safe) = l2_safe {
        println!("  Safe block:   {safe}");
    }

    // Oracle state (if address provided)
    if let Some(addr_str) = oracle_address {
        let oracle_addr: alloy_primitives::Address = addr_str.parse()?;
        use alloy_sol_types::SolCall;
        use open_zk_contracts::abi::IOpenZkL2OutputOracle;

        let call_data = IOpenZkL2OutputOracle::latestBlockNumberCall {}.abi_encode();
        let call_result = l1_provider
            .call(
                alloy_rpc_types_eth::TransactionRequest::default()
                    .to(oracle_addr)
                    .input(call_data.into()),
            )
            .await;

        println!("Oracle ({addr_str}):");
        match call_result {
            Ok(result) if result.len() >= 32 => {
                let bytes: [u8; 8] = result[24..32].try_into()?;
                let proven_block = u64::from_be_bytes(bytes);
                println!("  Latest proven block: {proven_block}");
                if let Some(safe) = l2_safe {
                    let pending = safe.saturating_sub(proven_block);
                    println!("  Pending blocks:      {pending}");
                }
            }
            Ok(_) => println!("  (unexpected response)"),
            Err(e) => println!("  (contract call failed: {e})"),
        }
    } else {
        println!();
        println!("Tip: pass --oracle-address to show proving progress.");
    }

    Ok(())
}
