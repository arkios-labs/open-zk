# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

open-zk is a Rust SDK for ZK-based applications. The initial focus is building a streamlined, cost-effective ZK proving solution for OP Stack rollups by unifying approaches from:

- **Kailua** (Boundless/RISC Zero): https://github.com/boundless-xyz/kailua
- **OP Succinct** (Succinct Labs): https://github.com/succinctlabs/op-succinct

Both use RISC-V ISA-based zkVMs (RISC Zero and SP1 respectively), enabling a backend-agnostic architecture where the zkVM can be swapped while keeping the proving workflow intact.

## Build & Development

This is a Rust workspace. Standard commands:

```
cargo build
cargo test
cargo test -p <crate-name>          # single crate
cargo test <test_name>              # single test
cargo clippy -- -D warnings         # lint
cargo fmt --check                   # format check
```
