#![no_main]
#![cfg_attr(not(test), no_std)]

sp1_zkvm::entrypoint!(main);

fn main() {
    guest_range_ethereum::guest_main(&open_zk_sp1_guest::Sp1Io);
}
