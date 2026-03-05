//! `open-zk prove` — Generate a proof for a block range.

use crate::config::CliConfig;
use clap::Args;
use std::path::PathBuf;
use tracing::info;

#[derive(Debug, Args)]
pub struct ProveArgs {
    /// Starting L2 block number (inclusive).
    #[arg(long)]
    pub start_block: u64,

    /// Ending L2 block number (inclusive).
    #[arg(long)]
    pub end_block: u64,

    /// Path to config file. Defaults to `open-zk.toml`.
    #[arg(long, short, default_value = "open-zk.toml")]
    pub config: PathBuf,
}

pub async fn execute(args: ProveArgs) -> anyhow::Result<()> {
    let config = CliConfig::from_file(&args.config)?;
    let sdk_config = config.to_sdk_config()?;
    let intent = sdk_config.resolve();
    let mock_mode = config.is_mock_mode();

    info!(
        start = args.start_block,
        end = args.end_block,
        mode = ?intent.proof_mode,
        backend = ?intent.backend,
        mock_mode,
        "starting proof generation"
    );

    // TODO: Wire up actual proving pipeline:
    // 1. Create WitnessProvider (RPC or mock)
    // 2. Create ProverBackend (SP1/RiscZero/mock)
    // 3. Generate witness for the block range
    // 4. Submit proof via ProofDispatcher
    // 5. Optionally submit on-chain via ProofSubmitter

    println!(
        "Proving blocks {}..{} with {:?} backend in {:?} mode",
        args.start_block, args.end_block, intent.backend, intent.proof_mode
    );
    println!("Proof generation not yet connected to live backends.");
    println!("Use `open-zk estimate` to get a cost estimate first.");

    Ok(())
}
