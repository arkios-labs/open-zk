# Contributing to open-zk

## Development Setup

### Prerequisites

- **Rust** (edition 2021, stable toolchain)
- **[just](https://github.com/casey/just)** — task runner
- **Docker** — for devnet
- **[Foundry](https://book.getfoundry.sh/)** — for Solidity tests
- **SP1 toolchain** (for SP1 guest builds):
  ```bash
  curl -L https://sp1.succinct.xyz | bash && sp1up --version 6.0.2
  ```
- **RISC Zero toolchain** (for RISC Zero guest builds):
  ```bash
  curl -L https://risczero.com/install | bash && rzup install
  ```

### Build & Test

```bash
cargo build                                        # Build workspace
cargo test --workspace                             # Run all unit tests
cargo test -p open-zk-core                         # Single crate
cargo test intent                                  # Filter by name
cargo clippy --workspace --all-targets -- -D warnings   # Lint
cargo fmt --check                                  # Format check
just ci                                            # All of the above
```

### Build Guest ELFs

```bash
# SP1
cd guests/range-ethereum/sp1 && cargo prove build --features sp1

# RISC Zero
cargo build -p open-zk-risc0 --features rebuild-guest,debug-guest-build
```

### Solidity Tests

```bash
cd contracts
forge install   # First time only
forge test
```

## CI Pipeline

GitHub Actions runs on every PR and push to `main`. All checks must pass before merging.

| Job | Description | Runs |
|-----|-------------|------|
| **Format** | `cargo fmt --check` | Parallel |
| **Clippy** | `cargo clippy --workspace -- -D warnings` | Parallel |
| **Test** | `cargo test --workspace` (unit tests) | Parallel |
| **Solidity** | `forge test` (contract tests) | Parallel |
| **E2E (SP1)** | SP1 mock prover against devnet | After Test |
| **E2E (RISC Zero)** | RISC Zero dev mode against devnet | After Test |

SP1 and RISC Zero E2E tests run in parallel on separate runners, each with its own devnet instance. Guest ELFs are cached by source hash to skip rebuilds when guest code hasn't changed.

## E2E Testing (Local)

Requires a running devnet:

```bash
just devnet-fetch   # One-time: clone Optimism monorepo
just devnet-up      # Start L1/L2/Beacon/OP Node

# SP1 E2E (mock prover, ~15s)
SP1_PROVER=mock cargo test -p open-zk-host \
  --features "sp1,kona" --test range_ethereum_e2e \
  --release -- --ignored --nocapture

# RISC Zero E2E (dev mode, ~15s)
RISC0_DEV_MODE=1 cargo test -p open-zk-host \
  --features "rebuild-risc0-guest,kona" --test range_ethereum_e2e \
  --release -- --ignored --nocapture

just devnet-down    # Stop containers
```

**Important**: Always use `--release` for E2E tests. Debug mode is ~14x slower.

## Style Guide

- Run `cargo fmt` before committing — CI enforces `cargo fmt --check`
- All clippy warnings are treated as errors
- Keep code simple — avoid over-engineering and premature abstractions

## Commit Messages

Follow the convention: `type(scope): description`

**Types**: `feat`, `fix`, `refactor`, `test`, `chore`, `docs`, `ci`, `style`

**Scope**: crate name (e.g., `host`, `guest`, `core`, `contracts`, `cli`, `orchestrator`)

Examples:
```
feat(host): add witness caching for repeated block ranges
fix(contracts): gate sol!(rpc) behind rpc feature
refactor(core): unify risczero naming to risc0
ci: split E2E into parallel SP1 and RISC Zero jobs
```

**Rules:**
1. Every commit must pass `cargo build && cargo test --workspace && cargo clippy -- -D warnings`
2. Do not mix unrelated changes (scaffolding + trait definitions = 2 separate commits)
3. Commit message explains "why" — the diff shows "what"

## Naming Conventions

- Feature flags, crate names, modules, filenames: `risc0` (matches official crate naming)
- Rust type names (structs, enums): `RiscZero*` (PascalCase brand name convention)

## Workspace Structure

Directory names are short; only `package.name` in Cargo.toml carries the `open-zk-` prefix:

```
core/          → package: open-zk-core         # Traits + types (no_std)
sdk/           → package: open-zk              # Public SDK (config, re-exports)
guest/         → package: open-zk-guest        # Guest-side zkVM I/O
host/          → package: open-zk-host         # Host-side prover backends
orchestrator/  → package: open-zk-orchestrator # Intent resolver + engine
onchain/       → package: open-zk-contracts    # On-chain ABI bindings
cli/           → package: open-zk-cli          # CLI binary
```

### Crate Dependency Direction

```
core  ←  guest
core  ←  host
core  ←  orchestrator
core + host + orchestrator  ←  sdk
sdk  ←  cli
core  ←  contracts
```

`core` depends on no other internal crate. Circular dependencies are forbidden.

### Feature Flags

- `core`: `std` (default) — host environment. Can be used with `no_std` inside guest (zkVM).
- `guest`: `sp1` | `risc0` — compile-time zkVM backend selection.
