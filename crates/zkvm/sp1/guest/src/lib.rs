use open_zk_core::traits::{ZkvmComposer, ZkvmReader, ZkvmWriter};
use serde::{de::DeserializeOwned, Serialize};

/// SP1 zkVM I/O adapter.
///
/// Delegates to `sp1_zkvm::io` functions. Only usable when compiled
/// for the SP1 zkVM target (`riscv64im-succinct-zkvm-elf`).
pub struct Sp1Io;

impl ZkvmReader for Sp1Io {
    fn read<T: DeserializeOwned>(&self) -> T {
        sp1_zkvm::io::read()
    }

    fn read_slice(&self, buf: &mut [u8]) {
        // SP1 v6 removed read_slice; use read_vec + copy instead.
        let vec = sp1_zkvm::io::read_vec();
        buf[..vec.len()].copy_from_slice(&vec);
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
        // SP1 v6 verify_sp1_proof takes a public values digest (SHA-256 hash).
        use sha2::{Digest, Sha256};
        let pv_digest: [u8; 32] = Sha256::digest(public_values).into();
        sp1_zkvm::lib::verify::verify_sp1_proof(vkey, &pv_digest);
    }
}
