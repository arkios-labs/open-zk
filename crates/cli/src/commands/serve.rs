//! `open-zk serve` — Run the proving loop as a long-running service.

use crate::config::CliConfig;
use clap::Args;
use std::path::PathBuf;
use tracing::info;

#[derive(Debug, Args)]
pub struct ServeArgs {
    /// Path to config file. Defaults to `open-zk.toml`.
    #[arg(long, short, default_value = "open-zk.toml")]
    pub config: PathBuf,

    /// Poll interval in seconds.
    #[arg(long, default_value = "12")]
    pub poll_interval: u64,
}

pub async fn execute(args: ServeArgs) -> anyhow::Result<()> {
    let config = CliConfig::from_file(&args.config)?;
    let sdk_config = config.to_sdk_config()?;
    let intent = sdk_config.resolve();

    info!(
        mode = ?intent.proof_mode,
        backend = ?intent.backend,
        poll_interval = args.poll_interval,
        "starting open-zk service"
    );

    println!("Starting open-zk service...");
    println!("Mode: {:?}", intent.proof_mode);
    println!("Backend: {:?}", intent.backend);
    println!("Poll interval: {}s", args.poll_interval);
    println!();

    // TODO: Wire up the OrchestrationEngine:
    // 1. Create ChainMonitor (RPC-based)
    // 2. Create ProofDispatcher (backend-specific)
    // 3. Create OrchestrationEngine with config + monitor + dispatcher
    // 4. Run engine.run() in a loop with graceful shutdown

    println!("Service loop not yet connected to live backends.");
    println!("Use mock testing via the SDK for now.");

    Ok(())
}
