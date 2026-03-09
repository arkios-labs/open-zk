#![cfg_attr(not(test), no_std)]

use open_zk_core::traits::{ZkvmComposer, ZkvmReader, ZkvmWriter};
use serde::{de::DeserializeOwned, Serialize};

/// RISC Zero zkVM I/O adapter.
///
/// Delegates to `risc0_zkvm::guest::env` functions. Only usable when
/// compiled for the RISC Zero zkVM target (`riscv32im-risc0-zkvm-elf`).
pub struct RiscZeroIo;

impl ZkvmReader for RiscZeroIo {
    fn read<T: DeserializeOwned>(&self) -> T {
        risc0_zkvm::guest::env::read()
    }

    fn read_slice(&self, buf: &mut [u8]) {
        risc0_zkvm::guest::env::read_slice(buf);
    }
}

impl ZkvmWriter for RiscZeroIo {
    fn commit<T: Serialize>(&self, value: &T) {
        risc0_zkvm::guest::env::commit(value);
    }

    fn commit_slice(&self, data: &[u8]) {
        risc0_zkvm::guest::env::commit_slice(data);
    }
}

impl ZkvmComposer for RiscZeroIo {
    type ProgramId = [u32; 8];

    fn verify_inner_proof(&self, image_id: &Self::ProgramId, journal_bytes: &[u8]) {
        risc0_zkvm::guest::env::verify(*image_id, journal_bytes)
            .expect("inner proof verification failed");
    }
}
