mod mock;

pub use mock::{MockProgram, MockProverBackend, MockWitness};

#[cfg(feature = "sp1")]
pub use open_zk_zkvm_sp1_host::{Sp1Program, Sp1ProverBackend, Sp1ProverError, Sp1Witness};

#[cfg(feature = "risc0")]
pub use open_zk_zkvm_risc0_host::{
    RiscZeroProgram, RiscZeroProverBackend, RiscZeroProverError, RiscZeroWitness,
};
