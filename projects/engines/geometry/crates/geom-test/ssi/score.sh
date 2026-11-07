#!/usr/bin/env bash
# Scores the stand-ins with the ssi-corpus's own scorer (piece G160; chapter 12.17):
#   score.sh <corpus> <runner>
# Run in the container the nightly workflow pins by digest. <corpus> is a writable copy of the
# corpus, with the failure catalogue in its cases/data; <runner> is ssi-runner, linked statically.
# Installs numpy and scipy from PyPI as the wheels pinned by SHA-256 below, then runs the corpus's
# scoring/score.py in isolated mode, once per stand-in and mode. Nothing here is a dependency of the
# kernel. numpy and scipy are BSD-3-Clause; the scorer is MIT or Apache-2.0.
set -euo pipefail
corpus=$1
runner=$2
venv=$(mktemp -d)
python3 -m venv "$venv"
req="$venv/requirements.txt"
# For CPython 3.13 on x86-64 manylinux, as PyPI lists them.
cat > "$req" <<'PINS'
numpy==2.5.4 --hash=sha256:6ffa07666f8da0eef81d149934a626d0d95fbd6838432a33e66245423a9062c0
scipy==1.18.1 --hash=sha256:fdaf5ea890a6183d0565f51a61799d67081bd5b1cf03c5f4b3fd3732108625c9
PINS
"$venv/bin/pip" install --quiet --disable-pip-version-check --no-deps --only-binary :all: \
  --require-hashes -r "$req" >&2
cd "$corpus"
while read -r stand_in mode; do
  echo "$stand_in $mode"
  SSI_ENGINE=$stand_in SSI_STAND_IN=$stand_in SSI_RUNNER=$runner \
    "$venv/bin/python" -I scoring/score.py "$mode" 1e-6 > "results/$stand_in-$mode.log"
done <<'RUNS'
truth certify
truth default
shifted default
spurious default
dropped default
flagged default
flagged certify
enlarged default
misplaced default
wrong-reason default
RUNS
