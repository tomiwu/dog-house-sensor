#!/usr/bin/env bash
# Quick checks for the Stop hook. LOOP_RANGE is the base..HEAD range.
set -euo pipefail
: "${LOOP_RANGE:=HEAD~1}"
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
