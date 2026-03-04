mod commands;
mod config;

use clap::{Parser, Subcommand};
use commands::{estimate, init, prove, serve, status};

/// open-zk: A cost-effective ZK proving solution for OP Stack rollups.
#[derive(Debug, Parser)]
#[command(name = "open-zk", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Generate a proof for a block range.
    Prove(prove::ProveArgs),
    /// Run the proving loop as a long-running service.
    Serve(serve::ServeArgs),
    /// Check the current proving status.
    Status(status::StatusArgs),
    /// Estimate proving cost for a block range.
    Estimate(estimate::EstimateArgs),
    /// Initialize a new open-zk.toml configuration file.
    Init(init::InitArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Prove(args) => prove::execute(args).await,
        Commands::Serve(args) => serve::execute(args).await,
        Commands::Status(args) => status::execute(args).await,
        Commands::Estimate(args) => estimate::execute(args).await,
        Commands::Init(args) => init::execute(args),
    }
}
