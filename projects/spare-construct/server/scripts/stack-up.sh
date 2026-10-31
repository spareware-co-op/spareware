#!/usr/bin/env bash
# Starts PD, TiKV and RustFS on this machine, for local testing. Data stays in $STACK_HOME/data.
# STACK_NODES=3 starts three each of PD and TiKV instead, as production has on three machines,
# each copy on its own ports. deploy/deploy.sh runs these programs as system services.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STACK_HOME="${STACK_HOME:-$HOME/spare-stack}"
STACK_NODES="${STACK_NODES:-1}"
RUSTFS_ACCESS_KEY="${RUSTFS_ACCESS_KEY:-testkey}"
RUSTFS_SECRET_KEY="${RUSTFS_SECRET_KEY:-testsecret}"
bin="$STACK_HOME/bin"
data="$STACK_HOME/data"
log="$STACK_HOME/log"
mkdir -p "$data/rustfs" "$log"

running() { pgrep -u "$(id -u)" -x "$1" >/dev/null; }
pd_client() { echo $((2379 + ($1 - 1) * 10)); }
pd_peer() { echo $((2380 + ($1 - 1) * 10)); }

# PD and TiKV stop cleanly on a hang-up signal, even under nohup, so each gets its own session.
if ! running pd-server; then
  cluster=""
  for node in $(seq 1 "$STACK_NODES"); do
    cluster="$cluster,pd$node=http://127.0.0.1:$(pd_peer "$node")"
  done
  for node in $(seq 1 "$STACK_NODES"); do
    mkdir -p "$data/pd$node"
    setsid nohup "$bin/pd-server" --name="pd$node" --data-dir="$data/pd$node" \
      --config="$here/config/pd.toml" --initial-cluster="${cluster#,}" \
      --client-urls="http://127.0.0.1:$(pd_client "$node")" \
      --peer-urls="http://127.0.0.1:$(pd_peer "$node")" \
      --log-file="$log/pd$node.log" >/dev/null 2>&1 &
  done
fi

if ! running tikv-server; then
  pds=""
  for node in $(seq 1 "$STACK_NODES"); do
    pds="$pds,127.0.0.1:$(pd_client "$node")"
  done
  for node in $(seq 1 "$STACK_NODES"); do
    mkdir -p "$data/tikv$node"
    setsid nohup "$bin/tikv-server" --pd-endpoints="${pds#,}" \
      --addr="127.0.0.1:$((20160 + node - 1))" --status-addr="127.0.0.1:$((20180 + node - 1))" \
      --data-dir="$data/tikv$node" --config="$here/config/tikv.toml" \
      --log-file="$log/tikv$node.log" >/dev/null 2>&1 &
  done
fi

if ! running rustfs; then
  RUSTFS_ACCESS_KEY="$RUSTFS_ACCESS_KEY" RUSTFS_SECRET_KEY="$RUSTFS_SECRET_KEY" \
    RUSTFS_ADDRESS=127.0.0.1:9100 RUSTFS_CONSOLE_ENABLE=false \
    setsid nohup "$bin/rustfs" "$data/rustfs" >"$log/rustfs.log" 2>&1 &
fi

stores() {
  curl -fs http://127.0.0.1:2379/pd/api/v1/stores 2>/dev/null | grep -c '"state_name": "Up"' || true
}
for _ in $(seq 1 90); do
  [ "$(stores)" -ge "$STACK_NODES" ] && break
  sleep 1
done
[ "$(stores)" -ge "$STACK_NODES" ] || { echo "TiKV did not come up; see $log" >&2; exit 1; }
if [ "$STACK_NODES" -gt 1 ]; then
  curl -fsS -X POST http://127.0.0.1:2379/pd/api/v1/config/replicate \
    -H 'content-type: application/json' -d "{\"max-replicas\": $STACK_NODES}" >/dev/null
fi

for _ in $(seq 1 30); do
  curl -s -o /dev/null http://127.0.0.1:9100/ && break
  sleep 1
done
for bucket in spare-files spare-backups; do
  curl -fsS -o /dev/null --aws-sigv4 "aws:amz:us-east-1:s3" \
    --user "$RUSTFS_ACCESS_KEY:$RUSTFS_SECRET_KEY" -X PUT "http://127.0.0.1:9100/$bucket" || true
done
echo "stack up: $STACK_NODES of PD and TiKV, RustFS on 9100"
