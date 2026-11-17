#!/usr/bin/env bash
# Deletes this machine's local test data and starts the stack empty. Local testing only: it asks
# nothing, and it never touches the backup folder.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STACK_HOME="${STACK_HOME:-$HOME/spare-stack}"

# The checklist wipes the stack partway through, so what TiKV measured until then is printed first.
if pgrep -u "$(id -u)" -x tikv-server >/dev/null; then bash "$here/store-sync-times.sh"; fi
bash "$here/stack-down.sh"
while pgrep -u "$(id -u)" -x 'tikv-server|pd-server|rustfs' >/dev/null; do sleep 0.2; done
rm -rf "$STACK_HOME"/data/pd* "$STACK_HOME"/data/tikv* "$STACK_HOME/data/rustfs"
if [ -n "${STACK_TIKV_RAMDISK:-}" ]; then rm -rf "$STACK_TIKV_RAMDISK"/tikv*; fi
bash "$here/stack-up.sh"
