#![no_main]
#![cfg_attr(not(any(feature = "sp1", test)), no_std)]

sp1_zkvm::entrypoint!(main);

fn main() {
    guest_range_ethereum::guest_main(open_zk_zkvm_sp1_guest::Sp1Io);
}
