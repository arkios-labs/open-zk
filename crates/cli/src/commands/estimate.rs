//! `open-zk estimate` — Estimate proving cost for a block range.

use crate::config::CliConfig;
use clap::Args;
use open_zk_core::traits::ProverBackend;
use open_zk_host::prover::{MockProgram, MockProverBackend, MockWitness};
use std::path::PathBuf;
use tracing::info;

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
    let mock_mode = config.is_mock_mode();

    let num_blocks = args.end_block.saturating_sub(args.start_block) + 1;
    let num_ranges = num_blocks.div_ceil(intent.aggregation_window);
    let needs_aggregation = num_ranges > 1;

    info!(
        start = args.start_block,
        end = args.end_block,
        backend = ?intent.backend,
        mock_mode,
        "starting cost estimation"
    );

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

    if mock_mode {
        println!("Mock mode: executing cost estimation with mock backend...");
        let backend = MockProverBackend;
        let program = MockProgram::new("range-ethereum");
        let witness = MockWitness::default();

        let estimate = backend.estimate_cost(&program, &witness).await?;
        print_estimate(&estimate, num_ranges, needs_aggregation);
        println!();
        println!("Note: mock estimates are zero — use a real backend for accurate costs.");
    } else {
        #[cfg(not(feature = "kona"))]
        anyhow::bail!(
            "cost estimation requires the `kona` feature — compile with: \
             cargo build --bin open-zk --features kona"
        );

        #[cfg(feature = "kona")]
        {
            println!("Generating witness from RPC nodes...");
            let witness =
                super::witness_helper::generate_witness(&config, args.start_block, args.end_block)
                    .await?;
            println!(
                "Witness generated: {} bytes oracle data",
                witness.oracle_data.len()
            );

            println!("Executing guest program to count cycles...");
            let start = std::time::Instant::now();
            let estimate = run_estimate(&intent, &witness).await?;
            let elapsed = start.elapsed();

            println!("Execution completed in {:.2}s", elapsed.as_secs_f64());
            println!();
            print_estimate(&estimate, num_ranges, needs_aggregation);
        }
    }

    Ok(())
}

fn print_estimate(
    estimate: &open_zk_core::types::CostEstimate,
    num_ranges: u64,
    needs_aggregation: bool,
) {
    println!("Per-range estimate:");
    println!("  Cycles:   {}", format_cycles(estimate.estimated_cycles));
    println!("  Cost:     ${:.4}", estimate.estimated_cost_usd);
    println!(
        "  Duration: {}",
        format_duration(estimate.estimated_duration_secs)
    );

    if needs_aggregation {
        let total_cycles = estimate.estimated_cycles * num_ranges;
        let total_cost = estimate.estimated_cost_usd * num_ranges as f64;
        let total_duration = estimate.estimated_duration_secs * num_ranges;

        println!();
        println!("Total estimate ({} ranges + aggregation):", num_ranges);
        println!("  Cycles:   {}", format_cycles(total_cycles));
        println!("  Cost:     ${:.4}", total_cost);
        println!(
            "  Duration: {} (sequential)",
            format_duration(total_duration)
        );
        println!();
        println!("  Note: Aggregation adds ~10-20% overhead. Ranges can be proven in parallel");
        println!("  to reduce wall-clock time.");
    } else {
        println!();
        println!("Total estimate (single range):");
        println!("  Cycles:   {}", format_cycles(estimate.estimated_cycles));
        println!("  Cost:     ${:.4}", estimate.estimated_cost_usd);
        println!(
            "  Duration: {}",
            format_duration(estimate.estimated_duration_secs)
        );
    }
}

fn format_cycles(cycles: u64) -> String {
    if cycles >= 1_000_000_000 {
        format!("{:.2}B", cycles as f64 / 1_000_000_000.0)
    } else if cycles >= 1_000_000 {
        format!("{:.2}M", cycles as f64 / 1_000_000.0)
    } else if cycles >= 1_000 {
        format!("{:.1}K", cycles as f64 / 1_000.0)
    } else {
        format!("{}", cycles)
    }
}

fn format_duration(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}s", secs)
    }
}

#[cfg(feature = "kona")]
async fn run_estimate(
    intent: &open_zk_orchestrator::ResolvedIntent,
    witness: &open_zk_core::traits::RawWitness,
) -> anyhow::Result<open_zk_core::types::CostEstimate> {
    use open_zk_core::types::ZkvmBackend;

    match intent.backend {
        ZkvmBackend::Mock => {
            let backend = MockProverBackend;
            let program = MockProgram::new("range-ethereum");
            let mock_witness = MockWitness::default();
            let estimate = backend.estimate_cost(&program, &mock_witness).await?;
            Ok(estimate)
        }
        #[cfg(feature = "sp1")]
        ZkvmBackend::Sp1 => {
            use open_zk_host::prover::{Sp1Program, Sp1ProverBackend, Sp1Witness};
            use sp1_sdk::SP1Stdin;

            let mut stdin = SP1Stdin::new();
            stdin.write(&witness.oracle_data);
            let sp1_witness = Sp1Witness { stdin };

            let elf = open_zk_host::include_range_ethereum_elf!();
            let program = Sp1Program::new("range-ethereum", elf.to_vec());
            let backend = Sp1ProverBackend::new().await;
            let estimate = backend.estimate_cost(&program, &sp1_witness).await?;
            Ok(estimate)
        }
        #[cfg(feature = "risc0")]
        ZkvmBackend::RiscZero => {
            use open_zk_host::prover::{RiscZeroProgram, RiscZeroProverBackend, RiscZeroWitness};

            let rz_witness = RiscZeroWitness {
                oracle_data: witness.oracle_data.clone(),
            };
            let elf = open_zk_host::elf::risc0::GUEST_RANGE_ETHEREUM_RISC0_ELF;
            let image_id = open_zk_host::elf::risc0::GUEST_RANGE_ETHEREUM_RISC0_ID;
            let program = RiscZeroProgram::new("range-ethereum", image_id, elf.to_vec());
            let backend = RiscZeroProverBackend::new();
            let estimate = backend.estimate_cost(&program, &rz_witness).await?;
            Ok(estimate)
        }
        #[allow(unreachable_patterns)]
        other => anyhow::bail!(
            "backend {:?} not available — compile with the corresponding feature flag",
            other
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_cycles_units() {
        assert_eq!(format_cycles(500), "500");
        assert_eq!(format_cycles(1_500), "1.5K");
        assert_eq!(format_cycles(2_500_000), "2.50M");
        assert_eq!(format_cycles(1_200_000_000), "1.20B");
    }

    #[test]
    fn format_duration_units() {
        assert_eq!(format_duration(30), "30s");
        assert_eq!(format_duration(90), "1m 30s");
        assert_eq!(format_duration(3661), "1h 1m");
    }
}
