#!/usr/bin/env bash
# Run the complete test suite without linking every diagnostic example twice.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

# --tests includes unit tests and integration tests, including explicit test=true targets.
# Finish the other test binaries after an assertion failure; repairs can rerun only failures.
cargo test --workspace --locked --tests --no-fail-fast "$@"
cargo test --workspace --locked --doc "$@"
# Examples are type-checked here; named acceptance scenarios build and execute them.
cargo check --workspace --locked --examples "$@"
