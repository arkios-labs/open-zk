# Contributing to open-zk

## Development Patterns

### Atomic Commits

Every commit contains **exactly one logical change unit**.

| Type | Prefix | Example |
|---|---|---|
| New feature | `feat:` | `feat: add ProverBackend trait` |
| Bug fix | `fix:` | `fix: correct intent resolution for economy mode` |
| Refactoring | `refactor:` | `refactor: rename crate directories to short names` |
| Test | `test:` | `test: add mock prover integration tests` |
| Documentation | `docs:` | `docs: update CLAUDE.md with new crate paths` |
| Infra/CI | `chore:` | `chore: add rust-toolchain.toml` |

**Rules:**
1. Every commit must pass `cargo build && cargo test && cargo clippy -- -D warnings`
2. Do not mix unrelated changes (scaffolding + trait definitions = 2 separate commits)
3. Commit message explains "why" — the diff shows "what"

### Workspace Structure

Directory names are short; only `package.name` in Cargo.toml carries the `open-zk-` prefix:

```
crates/
├── core/          → package: open-zk-core         # Traits + types (no_std)
├── sdk/           → package: open-zk              # Public SDK (config, re-exports)
├── guest/         → package: open-zk-guest        # Guest-side zkVM I/O
├── host/          → package: open-zk-host         # Host-side prover backends
├── orchestrator/  → package: open-zk-orchestrator # Intent resolver + engine
├── contracts/     → package: open-zk-contracts    # Solidity bindings
└── cli/           → package: open-zk-cli          # CLI binary
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

### Testing

```bash
cargo test                          # all crates
cargo test -p open-zk-core          # single crate
cargo test intent                   # filter by name
cargo clippy -- -D warnings         # lint (same as CI)
cargo fmt --check                   # format check
```

### Phase Roadmap

1. **Phase 1 — Foundation**: core traits/types, mock prover, config builder
2. **Phase 2 — SP1 Backend**: guest SP1 impl, Sp1ProverBackend, E2E
3. **Phase 3 — RISC Zero Backend**: guest RiscZero impl, same guest compiles on both
4. **Phase 4 — Orchestration**: aggregation guest, engine loops, scheduling
5. **Phase 5 — On-Chain**: Solidity contracts, bindings, deployment
6. **Phase 6 — CLI**: prove/serve/deploy commands, TOML config
