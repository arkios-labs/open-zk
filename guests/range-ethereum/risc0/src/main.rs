#![no_main]
#![cfg_attr(not(any(feature = "risc0", test)), no_std)]

risc0_zkvm::guest::entry!(main);

fn main() {
    guest_range_ethereum::guest_main();
}
