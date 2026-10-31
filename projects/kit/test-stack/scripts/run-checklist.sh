#!/usr/bin/env bash
# Runs the whole checklist from a clean start: wipes the local test data, seeds it, and runs
# tests/checklist.rs against the real stack. Set CARGO_TARGET_DIR to build outside this folder.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$here"
. config/local.env
target="${CARGO_TARGET_DIR:-$here/target}"

cargo build --release --bins --examples
pkill -u "$(id -u)" -x test-stack || true
bash scripts/stack-wipe.sh
if ! pgrep -u "$(id -u)" -x mock_oidc >/dev/null; then
  setsid nohup "$target/release/examples/mock_oidc" >/dev/null 2>&1 &
fi
sleep 1
"$target/release/test-stack" seed
cargo test --release --test checklist -- --ignored --nocapture --test-threads=1
