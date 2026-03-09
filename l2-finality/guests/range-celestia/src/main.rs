//! Celestia DA range proof guest program.
//!
//! Proves a range of L2 blocks using Celestia as the data availability layer.
//! Pipeline logic is shared via `run_range_program()`.

#![no_main]
#![cfg_attr(not(test), no_std)]

extern crate alloc;

mod da_source;

use alloc::sync::Arc;
use alloc::vec::Vec;
use da_source::CelestiaDataSource;
use kona_proof::l1::OracleL1ChainProvider;
use open_zk_core::traits::{ZkvmReader, ZkvmWriter};
use open_zk_guest::oracle::PreimageStore;
use open_zk_guest::pipeline::{DaSourceFactory, PreimageOracle};

#[cfg(feature = "sp1")]
sp1_zkvm::entrypoint!(main);

#[cfg(feature = "risc0")]
risc0_zkvm::guest::entry!(main);

struct CelestiaDa;

impl DaSourceFactory for CelestiaDa {
    type DA = CelestiaDataSource<OracleL1ChainProvider<PreimageOracle>, PreimageOracle>;

    fn create_da_source(
        &self,
        l1_provider: OracleL1ChainProvider<PreimageOracle>,
        oracle: Arc<PreimageOracle>,
        rollup_config: &kona_genesis::RollupConfig,
    ) -> Self::DA {
        CelestiaDataSource::new(l1_provider, oracle, rollup_config.batch_inbox_address)
    }
}

fn main() {
    #[cfg(feature = "sp1")]
    let io = open_zk_sp1_guest::Sp1Io;
    #[cfg(feature = "risc0")]
    let io = open_zk_risc0_guest::RiscZeroIo;

    let oracle_bytes: Vec<u8> = io.read();
    let store =
        PreimageStore::from_rkyv_bytes(&oracle_bytes).expect("failed to deserialize oracle data");
    open_zk_guest::pipeline::run(CelestiaDa, store, &io);
}
