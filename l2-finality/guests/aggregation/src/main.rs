//! Aggregation guest program.
//!
//! Verifies multiple range proofs and produces a single aggregated
//! proof covering the entire block range. Uses ZkvmComposer to
//! verify inner proofs within the guest.

#![no_main]
#![cfg_attr(not(test), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use open_zk_core::traits::{ZkvmComposer, ZkvmReader, ZkvmWriter};
use open_zk_core::types::StateTransitionJournal;

#[cfg(feature = "sp1")]
sp1_zkvm::entrypoint!(main);

#[cfg(feature = "risc0")]
risc0_zkvm::guest::entry!(main);

fn main() {
    #[cfg(feature = "sp1")]
    let io = open_zk_sp1_guest::Sp1Io;
    #[cfg(feature = "risc0")]
    let io = open_zk_risc0_guest::RiscZeroIo;

    // Read the number of range proofs and the expected program verification key
    let num_proofs: u32 = io.read();
    let range_program_vkey: [u32; 8] = io.read();

    // Verify each inner range proof and collect their journals
    let mut journals = Vec::new();
    for _ in 0..num_proofs {
        let journal_bytes: Vec<u8> = io.read();
        io.verify_inner_proof(&range_program_vkey, &journal_bytes);
        let journal = StateTransitionJournal::from_abi_bytes(&journal_bytes)
            .expect("invalid journal encoding");
        journals.push(journal);
    }

    assert!(!journals.is_empty(), "must have at least one proof");

    // Verify sequential continuity: each range's post_root must equal the next range's pre_root
    for w in journals.windows(2) {
        assert_eq!(
            w[0].l2_post_root, w[1].l2_pre_root,
            "discontinuity: post_root != next pre_root"
        );
        assert_eq!(
            w[0].rollup_config_hash, w[1].rollup_config_hash,
            "rollup config mismatch"
        );
    }

    // Build aggregated journal spanning first.pre_root → last.post_root
    let first = &journals[0];
    let last = &journals[journals.len() - 1];
    let aggregated = StateTransitionJournal {
        l1_head: first.l1_head,
        l2_pre_root: first.l2_pre_root,
        l2_post_root: last.l2_post_root,
        l2_block_number: last.l2_block_number,
        rollup_config_hash: first.rollup_config_hash,
        program_id: first.program_id,
    };

    let agg_bytes = aggregated.to_abi_bytes();
    io.commit_slice(&agg_bytes);
}
