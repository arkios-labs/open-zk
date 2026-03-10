//! `open-zk serve` — Run the proving loop as a long-running service.

use crate::config::CliConfig;
use clap::Args;
use open_zk_orchestrator::{
    EngineConfig, EngineEvent, MockDispatcher, MockMonitor, OrchestrationEngine,
};
use std::path::PathBuf;
use std::time::Duration;
use tracing::info;

#[derive(Debug, Args)]
pub struct ServeArgs {
    /// Path to config file. Defaults to `open-zk.toml`.
    #[arg(long, short, default_value = "open-zk.toml")]
    pub config: PathBuf,

    /// Poll interval in seconds.
    #[arg(long, default_value = "12")]
    pub poll_interval: u64,

    /// Oracle contract address on L1 (hex, 0x-prefixed).
    #[arg(long)]
    pub oracle_address: Option<String>,
}

pub async fn execute(args: ServeArgs) -> anyhow::Result<()> {
    let config = CliConfig::from_file(&args.config)?;
    let sdk_config = config.to_sdk_config()?;
    let intent = sdk_config.resolve();
    let mock_mode = config.is_mock_mode();

    info!(
        mode = ?intent.proof_mode,
        backend = ?intent.backend,
        poll_interval = args.poll_interval,
        mock_mode,
        "starting open-zk service"
    );

    println!("Starting open-zk service...");
    println!("  Mode:          {:?}", intent.proof_mode);
    println!("  Backend:       {:?}", intent.backend);
    println!("  Poll interval: {}s", args.poll_interval);
    if mock_mode {
        println!("  Mock mode:     ENABLED (no real ZK proofs)");
    }
    println!();

    let engine_config = EngineConfig {
        intent: intent.clone(),
        poll_interval: Duration::from_secs(args.poll_interval),
        max_concurrent_proofs: 4,
    };

    if mock_mode || args.oracle_address.is_none() {
        // Mock mode: use MockMonitor + MockDispatcher
        if args.oracle_address.is_none() && !mock_mode {
            println!("Warning: no --oracle-address provided, running in mock mode.");
            println!();
        }

        let monitor = MockMonitor::default();
        let mut engine = OrchestrationEngine::new(engine_config, monitor, MockDispatcher);
        let mut rx = engine.take_event_receiver();

        // Spawn event logger
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                log_event(&event);
            }
        });

        println!("Engine running (press Ctrl+C to stop)...");
        println!();

        tokio::select! {
            result = engine.run() => {
                result?;
            }
            _ = tokio::signal::ctrl_c() => {
                println!();
                println!("Shutting down gracefully...");
            }
        }
    } else {
        // RPC mode: use RpcChainMonitor + MockDispatcher
        let oracle_addr: alloy_primitives::Address =
            args.oracle_address.as_deref().unwrap().parse()?;

        let monitor = open_zk_orchestrator::rpc_monitor::RpcChainMonitor::new(
            config.network.l1_rpc_url.clone(),
            config.network.l2_rpc_url.clone(),
        );

        // The skeleton monitor's get_state() returns an error explaining the
        // `rpc` feature is needed, but we can still construct it to satisfy types.
        let _ = oracle_addr; // used when rpc feature is enabled

        let mut engine = OrchestrationEngine::new(engine_config, monitor, MockDispatcher);
        let mut rx = engine.take_event_receiver();

        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                log_event(&event);
            }
        });

        println!("Engine running with RPC monitor (press Ctrl+C to stop)...");
        println!();

        tokio::select! {
            result = engine.run() => {
                result?;
            }
            _ = tokio::signal::ctrl_c() => {
                println!();
                println!("Shutting down gracefully...");
            }
        }
    }

    Ok(())
}

fn log_event(event: &EngineEvent) {
    match event {
        EngineEvent::RangeDetected { start, end } => {
            println!("[engine] range detected: blocks {start}..{end}");
        }
        EngineEvent::ProofStarted { start, end } => {
            println!("[engine] proof started: blocks {start}..{end}");
        }
        EngineEvent::ProofCompleted { start, end } => {
            println!("[engine] proof completed: blocks {start}..{end}");
        }
        EngineEvent::ProofSubmitted { start, end } => {
            println!("[engine] proof submitted: blocks {start}..{end}");
        }
        EngineEvent::AggregationStarted { num_proofs } => {
            println!("[engine] aggregation started: {num_proofs} proofs");
        }
        EngineEvent::AggregationCompleted { start, end } => {
            println!("[engine] aggregation completed: blocks {start}..{end}");
        }
        EngineEvent::DisputeDetected { start, end } => {
            println!("[engine] dispute detected: blocks {start}..{end}");
        }
        EngineEvent::DisputeResolved { start, end } => {
            println!("[engine] dispute resolved: blocks {start}..{end}");
        }
        EngineEvent::Error { message } => {
            eprintln!("[engine] error: {message}");
        }
    }
}
