#!/usr/bin/env bash
# Starts a server on a fresh seed for tests/browser/two_browsers.mjs, and leaves it running. Run
# that test from any machine with Node and a Chromium browser that can reach localhost:8080.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$here"
. config/local.env
target="${CARGO_TARGET_DIR:-$here/target}"

cargo build --release -p spare-server --bins --examples
pkill -u "$(id -u)" -x spare-server || true
bash scripts/stack-wipe.sh
if ! pgrep -u "$(id -u)" -x mock_oidc >/dev/null; then
  setsid nohup "$target/release/examples/mock_oidc" >/dev/null 2>&1 &
fi
sleep 1
"$target/release/spare-server" seed
setsid nohup "$target/release/spare-server" serve >"${STACK_HOME:-$HOME/spare-stack}/log/server.log" 2>&1 &
sleep 1
curl -fsS http://127.0.0.1:8080/health
echo
