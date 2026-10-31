#!/usr/bin/env bash
# Holds the scored stand-ins to chapter 12.17's gate (piece G160):
#   gates.sh <corpus> <ssi>
# <corpus> is the folder score.sh wrote its results into, and <ssi> the harness's tool. The truth
# must pass both modes; each perturbed answer must break the rule it names, which is 12.17's
# mutation test; and the truth must replay bit for bit. The reports are the published result.
set -euo pipefail
corpus=$1
ssi=$2
cases=$corpus/cases/data

gate() {
  local engine=$1 mode=$2
  shift 2
  echo "== $engine $mode $*"
  "$ssi" gate --cases "$cases" --raw "$corpus/results/raw/${engine}_${mode}_1e-6" \
    --scores "$corpus/results/score_${engine}_${mode}_1e-6.json" \
    --engine "$engine" --mode "$mode" --tolerance 1e-6 \
    --report "$corpus/results/report_${engine}_${mode}.json" "$@"
}

gate truth certify
gate truth default
"$ssi" same "$corpus/results/report_truth_certify.json" "$corpus/results/report_truth_default.json"

gate shifted default --must-fail verdict
gate spurious default --must-fail verdict
gate dropped default --must-fail verdict

"$ssi" measure --cases "$cases" --raw "$corpus/results/raw/flagged_default_1e-6" \
  --out "$corpus/results/measured.json" --why "what the flagged stand-in needs"
gate flagged default --ceilings "$corpus/results/measured.json"
gate flagged default --must-fail over-ceiling
gate flagged certify --ceilings "$corpus/results/measured.json" --must-fail flagged-in-certify
gate enlarged default --ceilings "$corpus/results/measured.json" --must-fail too-wide
gate misplaced default --ceilings "$corpus/results/measured.json" --must-fail no-feature
gate wrong-reason default --ceilings "$corpus/results/measured.json" --must-fail wrong-kind
