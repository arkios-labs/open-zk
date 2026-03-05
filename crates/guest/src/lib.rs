#![cfg_attr(not(test), no_std)]

#[cfg(feature = "kona")]
pub mod oracle;

#[cfg(feature = "pipeline")]
pub mod pipeline;
