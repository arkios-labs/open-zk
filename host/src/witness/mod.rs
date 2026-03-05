mod adapter;
#[cfg(feature = "kona")]
pub mod kv_store;
mod mock;
#[cfg(feature = "kona")]
mod rpc;

pub use adapter::{bytes_to_raw_witness, raw_witness_to_bytes};
pub use mock::{MockWitnessError, MockWitnessProvider};
#[cfg(feature = "kona")]
pub use rpc::{RpcWitnessError, RpcWitnessProvider};

#[cfg(feature = "sp1")]
pub use open_zk_sp1::raw_witness_to_sp1_witness;

#[cfg(feature = "risc0")]
pub use open_zk_risc0::raw_witness_to_risc0_witness;
