#!/usr/bin/env bash
# Validates body records' payloads and geometry kinds against format/cddl/body.cddl with zcbor
# 0.9.1 (Apache-2.0, https://github.com/NordicSemiconductor/zcbor), which reads CDDL's tags (#6.n)
# as the test-only cddl-cat cannot. Run by kernel-nightly.yml's zcbor job in python:3.13-slim.
#
# Usage: zcbor.sh SCHEMA DIR. Every DIR/*.RULE.cbor file is validated as CDDL rule RULE; the
# script prints one line per file and fails if any does not validate.
#
# zcbor and its dependencies install into a fresh virtual environment from wheels pinned by
# SHA-256: cbor2 6.1.5 (MIT, as oracles/cbor2.sh in geom-op), PyYAML 6.0.3 (MIT) and regex
# 2026.9.29 (Apache-2.0 AND CNRI-Python).
set -euo pipefail
schema=$1
dir=$2
venv=$(mktemp -d)
python3 -m venv "$venv"
req="$venv/requirements.txt"
cat > "$req" <<'REQ'
zcbor==0.9.1 --hash=sha256:bc8bf4470f235c56d6276f162a5cd5417206335da21648ac2274f6417a18266e
cbor2==6.1.5 --hash=sha256:833db11fbea9808b080e5340d5f96615e28a6a6617618a4331e60082d0dc1ca4
pyyaml==6.0.3 --hash=sha256:0f29edc409a6392443abf94b9cf89ce99889a1dd5376d94316ae5145dfedd5d6
regex==2026.9.29 --hash=sha256:d49c18f1ea294cf4adde2e5ac256e98c82ea9d708462ce4bf799dffa7cfe8a2c
REQ
"$venv/bin/pip" install --quiet --disable-pip-version-check --no-deps --only-binary :all: \
  --require-hashes -r "$req" >&2
failed=0
count=0
for f in "$dir"/*.cbor; do
  rule=${f%.cbor}
  rule=${rule##*.}
  count=$((count + 1))
  if "$venv/bin/zcbor" validate -c "$schema" -t "$rule" -i "$f" --input-as cbor > /dev/null 2> "$venv/err"; then
    echo "ok $(basename "$f")"
  else
    echo "FAILED $(basename "$f"): $(tail -n 3 "$venv/err" | tr '\n' ' ')"
    failed=$((failed + 1))
  fi
done
echo "zcbor: $count file(s), $failed failed"
test "$count" -gt 0
test "$failed" -eq 0
