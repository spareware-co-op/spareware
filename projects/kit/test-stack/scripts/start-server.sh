#!/usr/bin/env bash
# Starts the built server in the background with the local settings.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$here"
. config/local.env
target="${CARGO_TARGET_DIR:-$here/target}"
setsid nohup "$target/release/test-stack" serve >>"${STACK_HOME:-$HOME/test-stack}/log/server.log" 2>&1 &
sleep 1
