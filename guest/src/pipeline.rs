//! Shared derivation pipeline for guest programs.
//!
//! Provides `DaSourceFactory` trait and `run_range_program()` to eliminate
//! code duplication across DA-specific guest programs. Each guest only needs
//! to implement the DA source creation; the pipeline orchestration is shared.

extern crate alloc;

use alloc::sync::Arc;
use alloy_consensus::Sealed;
use alloy_op_evm::OpEvmFactory;
use alloy_primitives::B256;
use kona_derive::DataAvailabilityProvider;
use kona_driver::Driver;
use kona_executor::TrieDBProvider;
use kona_preimage::{CommsClient, PreimageKey, PreimageKeyType};
use kona_proof::l1::{OracleL1ChainProvider, OraclePipeline};
use kona_proof::l2::OracleL2ChainProvider;
use kona_proof::sync::new_oracle_pipeline_cursor;
use kona_proof::{BootInfo, CachingOracle, HintType};
use open_zk_core::traits::ZkvmWriter;
use open_zk_core::types::StateTransitionJournal;

use crate::oracle::PreimageStore;

/// Type alias for the caching oracle backed by `PreimageStore`.
pub type PreimageOracle = CachingOracle<PreimageStore, PreimageStore>;

/// Factory trait for creating DA-specific data availability sources.
///
/// Each guest program implements this trait to provide its DA source
/// (Ethereum blobs, Celestia commitments, EigenDA, etc.). The shared
/// `run_range_program()` handles the rest of the pipeline.
pub trait DaSourceFactory {
    /// The data availability provider type.
    type DA: DataAvailabilityProvider + Send + Sync + core::fmt::Debug + Clone;

    /// Create the DA source from shared pipeline components.
    fn create_da_source(
        &self,
        l1_provider: OracleL1ChainProvider<PreimageOracle>,
        oracle: Arc<PreimageOracle>,
        rollup_config: &kona_genesis::RollupConfig,
    ) -> Self::DA;
}

/// Run the full kona derivation + execution pipeline.
///
/// This is the shared entry point for all guest programs. It handles:
/// 1. Oracle setup and boot info loading (prologue)
/// 2. DA source creation via the factory
/// 3. Pipeline construction, derivation, and execution
/// 4. Output root verification and journal commitment (epilogue)
pub fn run<F, IO>(factory: F, store: PreimageStore, io: &IO)
where
    F: DaSourceFactory,
    IO: ZkvmWriter,
{
    let (l1_head, l2_pre_root, l2_post_root, l2_block_number, config_hash) =
        kona_proof::block_on(run_pipeline(factory, store));

    let journal = StateTransitionJournal {
        l1_head,
        l2_pre_root,
        l2_post_root,
        l2_block_number,
        rollup_config_hash: config_hash,
        program_id: B256::ZERO,
    };
    io.commit_slice(&journal.to_abi_bytes());
}

async fn run_pipeline<F: DaSourceFactory>(
    factory: F,
    store: PreimageStore,
) -> (B256, B256, B256, u64, B256) {
    const ORACLE_LRU_SIZE: usize = 1024;

    // ================================================================
    //                          PROLOGUE
    // ================================================================

    let caching_oracle = Arc::new(CachingOracle::new(
        ORACLE_LRU_SIZE,
        store.clone(),
        store.clone(),
    ));

    let boot = BootInfo::load(caching_oracle.as_ref())
        .await
        .expect("failed to load boot info");

    let rollup_config = Arc::new(boot.rollup_config.clone());
    let l1_config = boot.l1_config.clone();

    let safe_head_hash = fetch_safe_head_hash(caching_oracle.as_ref(), boot.agreed_l2_output_root)
        .await
        .expect("failed to fetch safe head hash");

    let mut l1_provider = OracleL1ChainProvider::new(boot.l1_head, caching_oracle.clone());
    let mut l2_provider = OracleL2ChainProvider::new(
        safe_head_hash,
        rollup_config.clone(),
        caching_oracle.clone(),
    );

    let safe_head = l2_provider
        .header_by_hash(safe_head_hash)
        .map(|header| Sealed::new_unchecked(header, safe_head_hash))
        .expect("failed to fetch safe head header");

    assert!(
        boot.claimed_l2_block_number >= safe_head.number,
        "claimed L2 block number is less than the safe head"
    );

    // Trace extension: agreed == claimed means no work needed
    if boot.agreed_l2_output_root == boot.claimed_l2_output_root {
        let config_hash = B256::ZERO;
        return (
            boot.l1_head,
            boot.agreed_l2_output_root,
            boot.claimed_l2_output_root,
            boot.claimed_l2_block_number,
            config_hash,
        );
    }

    // ================================================================
    //                   DERIVATION & EXECUTION
    // ================================================================

    let cursor = new_oracle_pipeline_cursor(
        rollup_config.as_ref(),
        safe_head,
        &mut l1_provider,
        &mut l2_provider,
    )
    .await
    .expect("failed to create pipeline cursor");
    l2_provider.set_cursor(cursor.clone());

    // DA-specific source creation delegated to the factory
    let da_provider =
        factory.create_da_source(l1_provider.clone(), caching_oracle.clone(), &rollup_config);

    let pipeline = OraclePipeline::new(
        rollup_config.clone(),
        Arc::new(l1_config.into()),
        cursor.clone(),
        caching_oracle.clone(),
        da_provider,
        l1_provider.clone(),
        l2_provider.clone(),
    )
    .await
    .expect("failed to create derivation pipeline");

    let executor = kona_proof::executor::KonaExecutor::new(
        rollup_config.as_ref(),
        l2_provider.clone(),
        l2_provider,
        OpEvmFactory::default(),
        None,
    );

    let mut driver = Driver::new(cursor, executor, pipeline);

    let (_safe_head, output_root) = driver
        .advance_to_target(rollup_config.as_ref(), Some(boot.claimed_l2_block_number))
        .await
        .expect("failed to advance to target block");

    // ================================================================
    //                          EPILOGUE
    // ================================================================

    assert_eq!(
        output_root, boot.claimed_l2_output_root,
        "output root mismatch: derived {output_root} != claimed {}",
        boot.claimed_l2_output_root
    );

    let config_hash = B256::ZERO;
    (
        boot.l1_head,
        boot.agreed_l2_output_root,
        output_root,
        boot.claimed_l2_block_number,
        config_hash,
    )
}

/// Fetch the safe head block hash from the agreed L2 output root.
///
/// The L2 output root preimage is 128 bytes:
///   [version: 32][state_root: 32][withdrawal_storage_root: 32][block_hash: 32]
async fn fetch_safe_head_hash<O: CommsClient>(
    oracle: &O,
    agreed_l2_output_root: B256,
) -> Result<B256, kona_proof::errors::OracleProviderError> {
    let mut output_preimage = [0u8; 128];
    HintType::StartingL2Output
        .with_data(&[agreed_l2_output_root.as_ref()])
        .send(oracle)
        .await?;
    oracle
        .get_exact(
            PreimageKey::new(*agreed_l2_output_root, PreimageKeyType::Keccak256),
            output_preimage.as_mut(),
        )
        .await
        .map_err(kona_proof::errors::OracleProviderError::Preimage)?;
    output_preimage[96..128]
        .try_into()
        .map_err(kona_proof::errors::OracleProviderError::SliceConversion)
}
