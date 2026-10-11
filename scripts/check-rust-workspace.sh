#!/usr/bin/env bash
# Run the complete test suite without linking every diagnostic example twice.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

# --tests includes unit tests and integration tests, including explicit test=true targets.
cargo test --workspace --locked --tests "$@"
cargo test --workspace --locked --doc "$@"
# Examples are type-checked here; named acceptance scenarios build and execute them.
cargo check --workspace --locked --examples "$@"
