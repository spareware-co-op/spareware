#!/usr/bin/env bash
# Prints, from each local TiKV's own metrics, how many times it synced its Raft log and how many
# writes waited for a sync, with how many of each took over 0.1, 0.25 and 0.5 s. A slow edit in the
# checklist beside slow syncs here waited for the disk, not for the server (issue #886).
# run-checklist.sh prints this when it ends, pass or fail, and stack-wipe.sh before each wipe.
STACK_NODES="${STACK_NODES:-1}"
echo "TiKV's Raft log syncs since it started, and the writes that waited for them:"
for node in $(seq 1 "$STACK_NODES"); do
  port=$((20180 + node - 1))
  if ! metrics="$(curl -fs "http://127.0.0.1:$port/metrics")"; then
    echo "TiKV $node: not running, no sync times"
    continue
  fi
  # The buckets count every observation up to their limit, so the count over a limit is the
  # +Inf bucket's less that of the last bucket at or under the limit.
  awk -v node="$node" '
    /^(raft_engine_sync_log|tikv_raftstore_store_wf_persist)_duration_seconds_bucket\{le="/ {
      name = $1; sub(/_duration_seconds_bucket.*/, "", name)
      le = $1; sub(/.*le="/, "", le); sub(/".*/, "", le)
      if (le == "+Inf") { all[name] = $2; next }
      if (le + 0 <= 0.1) under1[name] = $2
      if (le + 0 <= 0.25) under25[name] = $2
      if (le + 0 <= 0.5) under5[name] = $2
    }
    END {
      for (name in all) {
        printf "TiKV %s %s: %d, over 0.1 s %d, over 0.25 s %d, over 0.5 s %d\n", node, name,
          all[name], all[name] - under1[name], all[name] - under25[name], all[name] - under5[name]
      }
    }' <<<"$metrics" | sort
done
