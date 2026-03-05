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

## E2E Testing Rules

- **Always use `--release`** for E2E tests. Debug mode is 14x slower.
- **Never run SP1 and RISC Zero E2E in parallel** — shared devnet deployer key causes nonce collisions.
- Devnet required: `just devnet-up` (L1=8545, L2=9545, Beacon=5052, OP Node=7545)

```bash
# SP1 E2E (mock prover, ~15s)
SP1_PROVER=mock cargo test -p open-zk-host \
  --features "sp1,kona" --test range_ethereum_e2e \
  --release -- --ignored --nocapture

# RISC Zero E2E (dev mode, ~15s)
RISC0_DEV_MODE=1 cargo test -p open-zk-host \
  --features "rebuild-risc0-guest,kona" --test range_ethereum_risc0_e2e \
  --release -- --ignored --nocapture

# On-chain E2E (requires deployed contracts)
ORACLE_ADDRESS=0x76ca03a67C049477FfB09694dFeF00416dB69746 \
  cargo test -p open-zk-contracts --features rpc \
  --test onchain_e2e --release -- --ignored --nocapture
```

## Naming Conventions

- Feature flags, crate names, modules, filenames: `risc0` (matches official crate naming)
- Rust type names (structs, enums): `RiscZero*` (PascalCase brand name convention)

## Style Guide

- Run `cargo fmt` before committing — CI enforces `cargo fmt --check`
- Run `cargo clippy --workspace --all-targets -- -D warnings` — CI treats warnings as errors
- Commit messages: `type(scope): description` (e.g., `fix(host): handle empty witness`)
  - Types: `feat`, `fix`, `refactor`, `test`, `chore`, `docs`, `ci`, `style`
  - Scope: crate name (`host`, `guest`, `core`, `contracts`, `cli`, `orchestrator`)

## Workflow Rules

- **Always run tests** (`cargo test --workspace`) after completing a task before considering it done.
- **Update README.md** if changes affect user-facing behavior, CLI commands, configuration, architecture, or setup instructions. README is for end users — keep it focused on usage, not internals.
- **Update CONTRIBUTING.md** if changes affect development setup, CI pipeline, commit conventions, or contributor workflows.
- **Update CLAUDE.md** if changes affect build commands, testing procedures, naming conventions, or workflow rules.
