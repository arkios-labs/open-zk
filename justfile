# open-zk — Justfile for devnet lifecycle and development tasks
#
# Prerequisites:
#   - just: https://github.com/casey/just
#   - docker & docker-compose: for devnet
#   - cargo: Rust toolchain
#   - (optional) SP1 toolchain: for guest ELF build

set fallback := true

# ── Build ──────────────────────────────────────────────────────────

# Build the CLI in release mode
build:
    cargo build --bin open-zk --release

# Build the CLI in debug mode with devnet defaults
devnet-build:
    cargo build --bin open-zk

# ── Devnet Lifecycle ───────────────────────────────────────────────
#
# The devnet uses the Optimism monorepo's Docker Compose setup.
# First run `just devnet-fetch` to clone the Optimism repo, then
# `just devnet-up` to start L1 + L2 + Beacon + OP Node containers.
#
# Default endpoints after devnet-up:
#   L1 RPC:       http://127.0.0.1:8545
#   L2 RPC:       http://127.0.0.1:9545
#   L1 Beacon:    http://127.0.0.1:5052
#   OP Node RPC:  http://127.0.0.1:7545

# Clone the Optimism monorepo (shallow, pinned to v1.9.1)
devnet-fetch:
    git clone --depth 1 --branch v1.9.1 --recursive https://github.com/ethereum-optimism/optimism.git

# Start the devnet (L1 + L2 + Beacon + OP Node)
devnet-up:
    make -C optimism devnet-up > devnet.log 2>&1

# Stop the devnet
devnet-down:
    make -C optimism devnet-down

# Stop and clean devnet state
devnet-clean: devnet-down
    make -C optimism devnet-clean

# Full reset: stop → clean → start
devnet-reset: devnet-down devnet-clean devnet-up

# ── Contract Deployment ────────────────────────────────────────────
#
# Deploys OpenZk contracts to the running devnet.
# Uses pre-funded devnet accounts from the Optimism monorepo.

# Devnet pre-funded account private keys (from OP Stack devnet genesis)
deployer_key := "0x4bbbf85ce3377467afe5d46f804f221813b2bb87f24d81f60f1fcdbf7cbf4356"
owner_key := "0x7c852118294e51e653712a81e05800f419141751be58f605c371e05141b007a6"

l1_rpc := "http://127.0.0.1:8545"
l2_rpc := "http://127.0.0.1:9545"
l1_beacon := "http://127.0.0.1:5052"
op_node_rpc := "http://127.0.0.1:7545"

# Deploy OpenZk contracts to devnet
devnet-deploy:
    cargo run --bin open-zk -- fast-track \
        --l1-rpc-url {{ l1_rpc }} \
        --l2-rpc-url {{ l2_rpc }} \
        --l1-beacon-url {{ l1_beacon }} \
        --deployer-key {{ deployer_key }} \
        --owner-key {{ owner_key }}

# ── Testing ────────────────────────────────────────────────────────

# Run all workspace tests
test:
    cargo test --workspace

# Run tests with all features
test-all:
    cargo test --workspace --all-features

# Run clippy lints
lint:
    cargo clippy --workspace -- -D warnings

# Run format check
fmt-check:
    cargo fmt --check

# Run devnet integration tests (requires running devnet)
test-devnet:
    SP1_PROVER=mock cargo test --workspace -- --ignored

# Full CI check: format + lint + test
ci: fmt-check lint test

# ── Guest ELF Build (SP1) ─────────────────────────────────────────
#
# Requires SP1 toolchain:
#   curl -L https://sp1.succinct.xyz | bash && sp1up

# Build the range proof guest ELF
guest-build-range:
    cd l2-finality/guests/range-ethereum/sp1 && cargo prove build

# Build the aggregation guest ELF
guest-build-aggregation:
    cd l2-finality/guests/aggregation && cargo prove build

# Build all guest ELFs
guest-build: guest-build-range guest-build-aggregation

# ── Full Devnet Workflow ───────────────────────────────────────────
#
# Complete workflow to bring up a devnet and run E2E tests:
#
#   just devnet-fetch    # one-time: clone optimism monorepo
#   just devnet-up       # start L1/L2/Beacon/OP-Node containers
#   just devnet-deploy   # deploy OpenZk contracts
#   just test-devnet     # run integration tests
#   just devnet-down     # stop containers
