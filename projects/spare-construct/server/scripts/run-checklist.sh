#!/usr/bin/env bash
# Runs everything that needs a real TiKV, from a clean start: wipes the local test data, seeds it,
# holds TiKV to the store's promises, runs the load test, then the acceptance checklist.
# Set CARGO_TARGET_DIR to build outside this folder, and LOAD_EDITS to change the load test's size.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$here"
. config/local.env
target="${CARGO_TARGET_DIR:-$here/target}"

services="$here/../../kit/services/Cargo.toml"

cargo build --release -p spare-server --bins --examples
pkill -u "$(id -u)" -x spare-server || true
bash scripts/stack-wipe.sh
if ! pgrep -u "$(id -u)" -x mock_oidc >/dev/null; then
  setsid nohup "$target/release/examples/mock_oidc" >/dev/null 2>&1 &
fi
sleep 1
"$target/release/spare-server" seed
cargo test --release --manifest-path "$services" -p kit-store-tikv --test promises -- --ignored --test-threads=1
cargo test --release --manifest-path "$services" -p kit-store-tikv --test load -- --ignored --nocapture
cargo test --release -p spare-server --test checklist -- --ignored --nocapture --test-threads=1
