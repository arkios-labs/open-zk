//! `open-zk prove` — Generate a proof for a block range.

use crate::config::CliConfig;
use clap::Args;
use open_zk_core::traits::ProverBackend;
#[cfg(feature = "kona")]
use open_zk_core::types::StateTransitionJournal;
use open_zk_host::prover::{MockProgram, MockProverBackend, MockWitness};
use std::path::PathBuf;
use std::time::Instant;
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

    println!(
        "Proving blocks {}..{} with {:?} backend ({:?} mode)",
        args.start_block, args.end_block, intent.backend, intent.proving_mode
    );

    if mock_mode {
        println!("Mock mode: generating dummy proof...");
        let backend = MockProverBackend;
        let program = MockProgram::new("range-ethereum");
        let witness = MockWitness::default();

        let start = Instant::now();
        let result = backend
            .prove(&program, &witness, intent.proving_mode)
            .await?;
        let elapsed = start.elapsed();

        println!();
        println!("Proof generated in {:.2}s", elapsed.as_secs_f64());
        println!("  Backend:      {:?}", result.backend);
        println!("  Mode:         {:?}", result.mode);
        println!("  Proof bytes:  {} bytes", result.proof_bytes.len());
        println!("  Cycle count:  {:?}", result.cycle_count);
        println!();
        println!("Note: mock proofs are not valid for on-chain submission.");
    } else {
        #[cfg(not(feature = "kona"))]
        anyhow::bail!(
            "real proving requires the `kona` feature — compile with: \
             cargo build --bin open-zk --features kona"
        );

        #[cfg(feature = "kona")]
        {
            // Real proving pipeline: witness generation → zkVM execution
            println!();
            println!("Generating witness from RPC nodes...");
            let witness = generate_witness(&config, args.start_block, args.end_block).await?;
            println!(
                "Witness generated: {} bytes oracle data",
                witness.oracle_data.len()
            );

            let start = Instant::now();
            let result = run_prover(&intent, &witness).await?;
            let elapsed = start.elapsed();

            println!();
            println!("Proof generated in {:.2}s", elapsed.as_secs_f64());
            println!("  Backend:       {:?}", result.backend);
            println!("  Mode:          {:?}", result.mode);
            println!("  Proof bytes:   {} bytes", result.proof_bytes.len());
            println!("  Public values: {} bytes", result.public_values.len());
            println!("  Cycle count:   {:?}", result.cycle_count);

            if !result.public_values.is_empty() {
                if let Ok(journal) = StateTransitionJournal::from_abi_bytes(&result.public_values) {
                    println!();
                    println!("Journal:");
                    println!("  l1_head:        {}", journal.l1_head);
                    println!("  l2_pre_root:    {}", journal.l2_pre_root);
                    println!("  l2_post_root:   {}", journal.l2_post_root);
                    println!("  l2_block_number: {}", journal.l2_block_number);
                }
            }
        }
    }

    Ok(())
}

#[cfg(feature = "kona")]
async fn generate_witness(
    config: &CliConfig,
    start_block: u64,
    end_block: u64,
) -> anyhow::Result<open_zk_core::traits::RawWitness> {
    use alloy_provider::{Provider, ProviderBuilder};
    use alloy_rpc_types_eth::BlockNumberOrTag;
    use open_zk_core::traits::WitnessProvider;
    use open_zk_core::types::{ProofRequest, ProvingMode};
    use open_zk_host::witness::RpcWitnessProvider;

    let l1_url: url::Url = config.network.l1_rpc_url.parse()?;
    let l1_provider = ProviderBuilder::new().connect_http(l1_url);

    // Fetch L1 head
    let l1_block = l1_provider
        .get_block_by_number(BlockNumberOrTag::Latest)
        .await?
        .ok_or_else(|| anyhow::anyhow!("L1 latest block not found"))?;
    let l1_head = l1_block.header.hash;

    // Fetch L2 output root at start block via OP Node
    let op_node_url = config
        .network
        .op_node_url
        .as_deref()
        .unwrap_or("http://127.0.0.1:7545");
    let op_node_provider = ProviderBuilder::new().connect_http(op_node_url.parse::<url::Url>()?);
    let resp: serde_json::Value = op_node_provider
        .raw_request(
            "optimism_outputAtBlock".into(),
            [format!("0x{:x}", start_block)],
        )
        .await?;
    let l2_start_output_root = resp["outputRoot"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("failed to get output root"))?
        .parse()?;

    let provider = RpcWitnessProvider::new(
        config.network.l1_rpc_url.clone(),
        config.network.l2_rpc_url.clone(),
        config.network.l1_beacon_url.clone(),
    )
    .with_op_node_url(op_node_url.to_string())
    .with_chain_id(config.network.chain_id.unwrap_or(901));

    let request = ProofRequest {
        l1_head,
        l2_start_block: start_block,
        l2_end_block: end_block,
        l2_start_output_root,
        mode: ProvingMode::Execute,
    };

    let witness = provider.generate_witness(&request).await?;
    Ok(witness)
}

#[cfg(feature = "kona")]
async fn run_prover(
    intent: &open_zk_orchestrator::ResolvedIntent,
    witness: &open_zk_core::traits::RawWitness,
) -> anyhow::Result<open_zk_core::types::ProofArtifact> {
    use open_zk_core::types::ZkvmBackend;

    match intent.backend {
        ZkvmBackend::Mock => {
            let backend = MockProverBackend;
            let program = MockProgram::new("range-ethereum");
            let mock_witness = MockWitness::default();
            let result = backend
                .prove(&program, &mock_witness, intent.proving_mode)
                .await?;
            Ok(result)
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
            let result = backend
                .prove(&program, &sp1_witness, intent.proving_mode)
                .await?;
            Ok(result)
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
            let result = backend
                .prove(&program, &rz_witness, intent.proving_mode)
                .await?;
            Ok(result)
        }
        #[allow(unreachable_patterns)]
        other => anyhow::bail!(
            "backend {:?} not available — compile with the corresponding feature flag",
            other
        ),
    }
}
