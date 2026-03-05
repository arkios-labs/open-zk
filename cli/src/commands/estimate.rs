//! `open-zk estimate` — Estimate proving cost for a block range.

use crate::config::CliConfig;
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct EstimateArgs {
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

pub async fn execute(args: EstimateArgs) -> anyhow::Result<()> {
    let config = CliConfig::from_file(&args.config)?;
    let sdk_config = config.to_sdk_config()?;
    let intent = sdk_config.resolve();

    let num_blocks = args.end_block.saturating_sub(args.start_block) + 1;
    let num_ranges = num_blocks.div_ceil(intent.aggregation_window);
    let needs_aggregation = num_ranges > 1;

    println!("Cost Estimate");
    println!("=============");
    println!(
        "Block range: {}..{} ({} blocks)",
        args.start_block, args.end_block, num_blocks
    );
    println!("Backend: {:?}", intent.backend);
    println!("Mode: {:?}", intent.proof_mode);
    println!("Range proofs needed: {}", num_ranges);
    println!("Aggregation needed: {}", needs_aggregation);
    println!();
    println!("Note: Detailed cost estimation requires running the execution");
    println!("pipeline to count cycles. Use `open-zk prove` with --dry-run");
    println!("(coming soon) for accurate estimates.");

    Ok(())
}
