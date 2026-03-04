//! `open-zk status` — Check the current proving status.

use crate::config::CliConfig;
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Path to config file. Defaults to `open-zk.toml`.
    #[arg(long, short, default_value = "open-zk.toml")]
    pub config: PathBuf,
}

pub async fn execute(args: StatusArgs) -> anyhow::Result<()> {
    let config = CliConfig::from_file(&args.config)?;
    let _sdk_config = config.to_sdk_config()?;

    println!("open-zk Status");
    println!("==============");
    println!();

    // TODO: Connect to chain monitor to fetch live state:
    // - Latest L1 block
    // - Latest proven L2 block
    // - Latest safe L2 block
    // - Pending blocks to prove
    // - Active disputes

    println!("L1 RPC: {}", config.network.l1_rpc_url);
    println!("L2 RPC: {}", config.network.l2_rpc_url);
    println!("Beacon: {}", config.network.l1_beacon_url);
    println!();
    println!("Status check requires live RPC connections.");
    println!("Ensure your RPC endpoints are configured in open-zk.toml.");

    Ok(())
}
