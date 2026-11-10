#!/usr/bin/env bash
# Runs the cbor2 oracle (piece G86; chapter 28.10's out-of-process oracles):
#   cbor2.sh < cases > answers
# Run in the container the nightly workflow pins by digest. Installs cbor2 6.1.5 (MIT) from PyPI as
# the one wheel pinned by its SHA-256 below, then runs cbor2.py, beside this script, in isolated
# mode. Nothing here is a dependency of the kernel; the oracle runs as its own process.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
venv=$(mktemp -d)
python3 -m venv "$venv"
req="$venv/requirements.txt"
# cbor2 6.1.5 for CPython 3.13 on manylinux_2_28 x86-64, as PyPI lists it.
echo "cbor2==6.1.5 --hash=sha256:833db11fbea9808b080e5340d5f96615e28a6a6617618a4331e60082d0dc1ca4" > "$req"
"$venv/bin/pip" install --quiet --disable-pip-version-check --no-deps --only-binary :all: \
  --require-hashes -r "$req" >&2
"$venv/bin/python" -I "$here/cbor2.py"
