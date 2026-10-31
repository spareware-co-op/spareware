#!/usr/bin/env bash
# Deletes this machine's local test data and starts the stack empty. Local testing only: it asks
# nothing, and it never touches the backup folder.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STACK_HOME="${STACK_HOME:-$HOME/test-stack}"

bash "$here/stack-down.sh"
while pgrep -u "$(id -u)" -x 'tikv-server|pd-server|rustfs' >/dev/null; do sleep 0.2; done
rm -rf "$STACK_HOME"/data/pd* "$STACK_HOME"/data/tikv* "$STACK_HOME/data/rustfs"
bash "$here/stack-up.sh"
