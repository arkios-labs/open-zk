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

#[cfg(feature = "sp1")]
sp1_zkvm::entrypoint!(main);

// RISC Zero entry is handled by risc0_zkvm::guest::entry! macro
// which requires a different invocation pattern.
#[cfg(feature = "risczero")]
risc0_zkvm::guest::entry!(main);

use open_zk_core::types::StateTransitionJournal;
use open_zk_guest::io;

fn main() {
    let _io = io();

    // Phase 2/3 TODO: integrate kona derivation pipeline
    //
    // 1. Read boot info (L1 head, L2 pre-root, block range, rollup config)
    //    let boot_info: BootInfo = _io.read();
    //
    // 2. Run OP Stack derivation (kona-derive)
    //    let batches = derive_batches(&boot_info);
    //
    // 3. Execute L2 blocks (kona-executor / revm)
    //    let post_root = execute_blocks(&batches);
    //
    // 4. Commit the unified journal
    //    _io.commit(&StateTransitionJournal {
    //        l1_head: boot_info.l1_head,
    //        l2_pre_root: boot_info.l2_pre_root,
    //        l2_post_root: post_root,
    //        l2_block_number: boot_info.l2_end_block,
    //        rollup_config_hash: boot_info.rollup_config_hash,
    //        program_id: boot_info.program_id,
    //    });
}
