//! Ethereum DA range proof guest program (RISC Zero variant).
//!
//! Proves a range of L2 blocks using Ethereum (calldata + blobs) as the
//! data availability layer. Same logic as guests/range-ethereum/ but built
//! for the RISC Zero zkVM with patched crates.

#![no_main]
#![cfg_attr(not(any(feature = "risc0", test)), no_std)]

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use kona_derive::EthereumDataSource;
use kona_proof::l1::{OracleBlobProvider, OracleL1ChainProvider};
use open_zk_core::traits::ZkvmReader;
use open_zk_guest::oracle::PreimageStore;
use open_zk_guest::pipeline::{DaSourceFactory, PreimageOracle};

risc0_zkvm::guest::entry!(main);

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

fn main() {
    let io = open_zk_guest::io();
    let oracle_bytes: Vec<u8> = io.read();
    let store =
        PreimageStore::from_rkyv_bytes(&oracle_bytes).expect("failed to deserialize oracle data");
    open_zk_guest::pipeline::run(EthereumDa, store, &io);
}
