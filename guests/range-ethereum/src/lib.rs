//! Shared logic for the Ethereum DA range proof guest program.
//!
//! Proves a range of L2 blocks using Ethereum (calldata + blobs) as the
//! data availability layer. zkVM-specific entrypoints live in sp1/ and risc0/.

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use kona_derive::EthereumDataSource;
use kona_proof::l1::{OracleBlobProvider, OracleL1ChainProvider};
use open_zk_core::traits::{ZkvmReader, ZkvmWriter};
use open_zk_guest::oracle::PreimageStore;
use open_zk_guest::pipeline::{DaSourceFactory, PreimageOracle};

struct EthereumDa;

impl DaSourceFactory for EthereumDa {
    type DA = EthereumDataSource<
        OracleL1ChainProvider<PreimageOracle>,
        OracleBlobProvider<PreimageOracle>,
    >;

    fn create_da_source(
        &self,
        l1_provider: OracleL1ChainProvider<PreimageOracle>,
        oracle: Arc<PreimageOracle>,
        rollup_config: &kona_genesis::RollupConfig,
    ) -> Self::DA {
        let beacon = OracleBlobProvider::new(oracle);
        EthereumDataSource::new_from_parts(l1_provider, beacon, rollup_config)
    }
}

pub fn guest_main(io: impl ZkvmReader + ZkvmWriter) {
    let oracle_bytes: Vec<u8> = io.read();
    let store =
        PreimageStore::from_rkyv_bytes(&oracle_bytes).expect("failed to deserialize oracle data");
    open_zk_guest::pipeline::run(EthereumDa, store, &io);
}
