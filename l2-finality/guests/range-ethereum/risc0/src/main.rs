#![no_main]
#![cfg_attr(not(test), no_std)]

risc0_zkvm::guest::entry!(main);

fn main() {
    guest_range_ethereum::guest_main(&open_zk_risc0_guest::RiscZeroIo);
}
