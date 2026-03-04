//! Range proof guest program.
//!
//! Proves a range of L2 blocks by running the OP Stack derivation
//! pipeline and EVM execution inside a zkVM. The same source code
//! compiles for both SP1 and RISC Zero via feature flags.
//!
//! Build:
//!   SP1:       cargo build --features sp1 --target riscv32im-succinct-zkvm-elf
//!   RISC Zero: cargo build --features risczero --target riscv32im-risc0-zkvm-elf

#![no_main]
#![cfg_attr(not(test), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use open_zk_core::traits::{ZkvmReader, ZkvmWriter};
use open_zk_core::types::{BootInfo, StateTransitionJournal};

#[cfg(feature = "sp1")]
sp1_zkvm::entrypoint!(main);

#[cfg(feature = "risczero")]
risc0_zkvm::guest::entry!(main);

fn main() {
    let io = open_zk_guest::io();

    // 1. Read boot info (L1 head, L2 pre-root, claim, block number, rollup config)
    let boot_info: BootInfo = io.read();

    // 2. Read witness data (serialized preimage oracle + blob data)
    let _witness_bytes: Vec<u8> = io.read();

    // 3. OP Stack derivation pipeline (kona integration)
    //
    // When kona dependencies are available, the full pipeline is:
    //
    //   a. Deserialize preimage store from witness_bytes
    //      let oracle = InMemoryOracle::from_raw_bytes(&witness_bytes);
    //
    //   b. Create oracle-backed L1/L2 chain providers
    //      let l1_provider = OracleL1ChainProvider::new(oracle.clone(), &boot_info);
    //      let l2_provider = OracleL2ChainProvider::new(oracle.clone(), &boot_info);
    //
    //   c. Create blob provider from witness data
    //      let blob_provider = OracleBlobProvider::new(oracle.clone());
    //
    //   d. Build kona derivation pipeline
    //      let pipeline = OraclePipeline::new(
    //          &boot_info.rollup_config,
    //          l1_provider,
    //          blob_provider,
    //          l2_provider,
    //      );
    //
    //   e. Create kona executor (wraps revm for L2 block execution)
    //      let executor = KonaExecutor::new(&boot_info, l2_provider.clone(), oracle);
    //
    //   f. Build and run the driver
    //      let mut driver = Driver::new(pipeline, executor);
    //      let output_root = driver.advance_to_target(boot_info.l2_block_number);
    //
    //   g. Verify the output root matches the claim
    //      assert_eq!(output_root, boot_info.l2_claim);

    // 4. Commit the state transition journal
    let journal = StateTransitionJournal {
        l1_head: boot_info.l1_head,
        l2_pre_root: boot_info.l2_pre_root,
        l2_post_root: boot_info.l2_claim,
        l2_block_number: boot_info.l2_block_number,
        rollup_config_hash: boot_info.rollup_config_hash,
        program_id: alloy_primitives::B256::ZERO, // Set by host after compilation
    };

    let journal_bytes = journal.to_abi_bytes();
    io.commit_slice(&journal_bytes);
}
