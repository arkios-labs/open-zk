//! Shared witness generation logic for CLI commands.

#[cfg(feature = "kona")]
pub async fn generate_witness(
    config: &crate::config::CliConfig,
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
