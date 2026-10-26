#!/usr/bin/env bash
# Piece G35's FPTaylor pipeline, run inside the pinned OCaml 4.13 container
# (kernel-certificates.yml): builds FPTaylor v0.9.3, the last release that records proof
# certificates, asks it for each problem's error bound with a certificate, builds HOL Light, and
# has HOL Light check the certificate against the filter's bound.
# Usage: fptaylor.sh <checks.txt>, each line `<problem.fpt> <expression> <bound in HOL Light syntax>`.
#
# OCaml 4.13, not 4.14: FPTaylor saves certificates as marshalled `Num` values, and HOL Light uses
# the Num library below OCaml 4.14 and Zarith from 4.14 on (its Makefile), so both must be 4.13.
set -euo pipefail

FPTAYLOR_COMMIT=f85a7a7797990fe5fb3436071afaa72c3f05629b    # v0.9.3 (MIT)
HOL_LIGHT_COMMIT=d30e8239887b2348a063f82232a864de1cc2df15   # 2026-10-20 (BSD-style licence)

here=$(cd "$(dirname "$0")" && pwd)
checks=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
work=$(mktemp -d)

# The image's user installs system packages through sudo; a root user needs none.
SUDO=$(command -v sudo || true)
$SUDO apt-get update -qq
$SUDO env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq git gcc make m4 pkg-config libpcre2-dev >/dev/null
eval "$(opam env)"
opam update -q >/dev/null
opam install -y num camlp5 ocamlfind >/dev/null
opam list --installed | grep -E '^(ocaml|num|camlp5|ocamlfind) '
ocaml -version
camlp5 -v 2>&1 | head -n 1

fetch() {
  git init -q "$2"
  git -C "$2" fetch -q --depth 1 "$1" "$3"
  git -C "$2" checkout -q FETCH_HEAD
}
fetch https://github.com/soarlab/FPTaylor.git "$work/FPTaylor" "$FPTAYLOR_COMMIT"
fetch https://github.com/jrh13/hol-light.git "$work/hol-light" "$HOL_LIGHT_COMMIT"

echo "::group::Build FPTaylor"
make -C "$work/FPTaylor" >/dev/null
echo "::endgroup::"

# FPTaylor writes certificates to proofs/ under the directory it runs in.
run="$work/run"
mkdir -p "$run"
cp "$here/fptaylor.cfg" "$run/"
while read -r problem expression bound; do
  [ -z "$problem" ] && continue
  echo "::group::FPTaylor on $problem"
  (cd "$run" && "$work/FPTaylor/fptaylor" -c fptaylor.cfg "$here/$problem")
  echo "::endgroup::"
done <"$checks"
ls -l "$run/proofs"

echo "::group::Build HOL Light"
make -C "$work/hol-light" >/dev/null
echo "::endgroup::"

# The HOL Light session: load HOL Light, its multivariate analysis, the inequality verifier
# bundled with it (Formal_ineqs) and FPTaylor's checker, then check each certificate against
# its bound. FPTaylor's formal/load.hl, with this machine's paths.
session="$work/session.ml"
{
  echo "let formal_ineqs_path = \"$work/hol-light/Formal_ineqs\";;"
  echo "let fptaylor_path = \"$work/FPTaylor\";;"
  echo 'let fptaylor_formal_path = fptaylor_path ^ "/formal";;'
  echo 'load_path := formal_ineqs_path :: fptaylor_formal_path :: !load_path;;'
  echo 'needs "arith_options.hl";;'
  echo 'Arith_options.base := 100;;'
  echo 'Arith_options.min_exp := 300;;'
  echo 'needs "Multivariate/realanalysis.ml";;'
  echo 'needs "verifier/m_verifier_main.hl";;'
  echo 'needs "build_tform2.hl";;'
  echo 'needs "tests/utils.hl";;'
  echo "path := \"$run/proofs\";;"
  while read -r problem expression bound; do
    [ -z "$problem" ] && continue
    echo "let th, time = validate_bound 10 \"$expression.proof\" \`$bound\`;;"
    echo 'print_endline "THEOREM:";; print_thm th;; print_newline ();;'
    echo "Printf.printf \"checked $expression in %.0f s\\n\" time;;"
  done <"$checks"
} >"$session"
cat "$session"

export HOLLIGHT_DIR="$work/hol-light"
"$work/hol-light/ocaml-hol" -I "$(camlp5 -where)" camlp5o.cma -init "$work/hol-light/hol.ml" \
  -safe-string -I "$work/hol-light" <"$session" | tee "$work/hol.log" | grep -v '^val \|^- : ' || true
grep -q '^checked ' "$work/hol.log"
