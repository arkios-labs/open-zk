//! `open-zk init` — Initialize a new open-zk configuration file.

use crate::config::CliConfig;
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Output path for the config file.
    #[arg(long, short, default_value = "open-zk.toml")]
    pub output: PathBuf,

    /// Overwrite existing config file.
    #[arg(long)]
    pub force: bool,
}

pub fn execute(args: InitArgs) -> anyhow::Result<()> {
    if args.output.exists() && !args.force {
        anyhow::bail!(
            "Config file already exists at {}. Use --force to overwrite.",
            args.output.display()
        );
    }

    let toml_content = CliConfig::default_toml();
    std::fs::write(&args.output, &toml_content)?;
    println!("Created config file at {}", args.output.display());
    println!("Edit the [network] section with your RPC URLs to get started.");

    Ok(())
}
