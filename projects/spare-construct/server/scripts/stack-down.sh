#!/usr/bin/env bash
# Stops what stack-up.sh started. Data stays. TiKV can wait forever for a PD that has gone, so
# anything still running after ten seconds is stopped the hard way.
me="$(id -u)"
pkill -u "$me" -x rustfs
pkill -u "$me" -x tikv-server
sleep 2
pkill -u "$me" -x pd-server
for _ in $(seq 1 20); do
  pgrep -u "$me" -x 'tikv-server|pd-server|rustfs' >/dev/null || exit 0
  sleep 0.5
done
pkill -9 -u "$me" -x 'tikv-server|pd-server|rustfs'
exit 0
