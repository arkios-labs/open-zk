use open_zk_core::traits::{ZkvmComposer, ZkvmReader, ZkvmWriter};
use serde::{de::DeserializeOwned, Serialize};

/// SP1 zkVM I/O adapter.
///
/// Delegates to `sp1_zkvm::io` functions. Only usable when compiled
/// for the SP1 zkVM target (`riscv32im-succinct-zkvm-elf`).
pub struct Sp1Io;

impl ZkvmReader for Sp1Io {
    fn read<T: DeserializeOwned>(&self) -> T {
        sp1_zkvm::io::read()
    }

    fn read_slice(&self, buf: &mut [u8]) {
        sp1_zkvm::io::read_slice(buf);
    }
}

impl ZkvmWriter for Sp1Io {
    fn commit<T: Serialize>(&self, value: &T) {
        sp1_zkvm::io::commit(value);
    }

    fn commit_slice(&self, data: &[u8]) {
        sp1_zkvm::io::commit_slice(data);
    }
}

impl ZkvmComposer for Sp1Io {
    type ProgramId = [u32; 8];

    fn verify_inner_proof(&self, vkey: &Self::ProgramId, public_values: &[u8]) {
        sp1_zkvm::lib::verify::verify_sp1_proof(vkey, &public_values.into());
    }
}
