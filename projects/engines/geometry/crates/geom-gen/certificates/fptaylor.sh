#!/usr/bin/env bash
# The FPTaylor pipeline of the filter certificates (chapter 28.6; pieces G35 and G40), run inside
# the pinned OCaml 4.11 container by kernel-certificates.yml: builds FPTaylor v0.9.3, the last
# release that records proof certificates, asks it for each problem's error bound with a
# certificate, builds HOL Light, has HOL Light check each certificate, and reports whether the
# bound it proves is within the one the problem names in its `// hol-light-bound:` line (K).
# A problem's `// fptaylor-optimisation:` line says how FPTaylor bounds it: `approx`, each error
# term on its own, whose bounds HOL Light checks term by term; or `exact`, the sum of the terms at
# once, which HOL Light proves at most the problem's `// hol-light-total:`, the largest double at
# most K, when FPTaylor's own bound is at most that (a larger target never needs more cells).
# The job fails if a certificate does not check; a certificate whose bound is above K is reported,
# not failed: FPTaylor's bound is its own, rounded up to a double, and can exceed a K that Gappa
# proves (the method note).
# A problem's `// hol-light-shards: n` line (geom-gen's SHARDED) has HOL Light check its terms in
# n jobs, all of one certificate, with `// hol-light-terms: t`, its number of terms, and a
# `// hol-light-shard k: ...` line for each shard naming the terms it proves (geom-gen's test holds
# every term 1 to t to exactly one shard): FPTAYLOR_OUT=<dir> runs FPTaylor alone and writes each
# problem's certificate there as <problem>.proof; FPTAYLOR_SHARD=k with FPTAYLOR_CERTIFICATES=<dir>
# checks one problem's certificate from that folder, proving the terms shard k names and assuming
# the others, and fails unless the certificate has t terms and it proved exactly those. Shards 1
# to n, each green, prove every term of the one certificate; their theorems are its bound under
# the terms the others proved.
# A term too costly for one job is cut into pieces: `// hol-light-cut t: d` has term t checked as
# 2^d pieces, its box halved d times as HOL Light's own search would first halve it, named t.1 to
# t.2^d on the shard lines (fptaylor_cut.hl, which this script puts into FPTaylor's checker). A
# shard with pieces of t proves its pieces, assumes the others, and proves t from them, so every
# piece in some green shard proves t; the check below holds it to exactly its pieces.
# A shard can start from a saved state of HOL Light instead of loading it (about an hour on a
# runner): FPTAYLOR_STATE_OUT=<dir> with one problem loads HOL Light, its multivariate analysis and
# the inequality verifier with that problem's Taylor tables, as a shard's session does, saves the
# process with DMTCP (HOL Light's README names it, and HOL Light's own make-checkpoint.sh uses it)
# to <dir>/hol-light-state.tar and prints the file's sha256; FPTAYLOR_STATE=<dir> with
# FPTAYLOR_STATE_SHA256=<that sha256>, for a shard, refuses a file with any other digest, restores
# the process from it and sends it the rest of the session: FPTaylor's checker, loaded as written
# with the shard's terms, and the certificate's check. The saved state holds no theorem the check
# relies on: every term the shard names is still proved after the restore. Both runs work in one
# fixed folder, so that the paths the saved process holds are the shard's.
# A cut term's pieces are one function on different boxes, and the verifier builds the function's
# Taylor theorems before it checks any box (for line_cone's term 38, longer than a piece takes).
# FPTAYLOR_CUT_STATE_OUT=<dir>, with FPTAYLOR_STATE, its sha256 and FPTAYLOR_CERTIFICATES, restores
# the loaded state, loads the checker, has the verifier build the Taylor theorems of each term the
# problem cuts (fptaylor_cache.hl keeps them; fptaylor_cut.hl's cut_setup), proving no term and no
# piece, and saves the process again, to <dir>/hol-light-state.tar, printing its sha256. A shard
# with pieces restores that state as it would the first, and proves its pieces as before: what the
# state adds is the function's theorems, which HOL Light's kernel proved in the saving run.
# Usage: fptaylor.sh <problem.fpt>...  (geom-gen writes the problems: generated/fptaylor-*.fpt)
#
# OCaml 4.11: FPTaylor saves certificates as marshalled `Num` values, and HOL Light uses the Num
# library below OCaml 4.14 and Zarith from 4.14 on (its Makefile); and FPTaylor's 2020 checker
# calls `compare` where an opened `List` shadows it with `List.compare` from OCaml 4.12 on
# (formal/build_tform2.hl, line 534). Below 4.12 the checker runs as written.
set -euo pipefail

FPTAYLOR_COMMIT=f85a7a7797990fe5fb3436071afaa72c3f05629b    # v0.9.3 (MIT)
HOL_LIGHT_COMMIT=d30e8239887b2348a063f82232a864de1cc2df15   # 2026-10-20 (BSD-style licence)
# DMTCP saves and restores the HOL Light process. It runs as programs of its own around HOL Light's
# toplevel, never in the kernel; built from source at this commit, in the container.
DMTCP_COMMIT=f8009ce7b4ad211311ca2f72a929b975e4aa1155       # v4.2.0 (LGPL-3.0-or-later)

here=$(cd "$(dirname "$0")" && pwd)

out=${FPTAYLOR_OUT:-}
certificates=${FPTAYLOR_CERTIFICATES:-}
shard=${FPTAYLOR_SHARD:-}
state_out=${FPTAYLOR_STATE_OUT:-}
cut_state_out=${FPTAYLOR_CUT_STATE_OUT:-}
state=${FPTAYLOR_STATE:-}
state_sha256=${FPTAYLOR_STATE_SHA256:-}
shards=1
if [ -n "$state_out" ]; then
  [ $# -eq 1 ] && [ -z "$out$certificates$shard$state$cut_state_out" ] ||
    { echo "FPTAYLOR_STATE_OUT saves the state for one problem, and does nothing else"; exit 1; }
fi
if [ -n "$cut_state_out" ]; then
  [ $# -eq 1 ] && [ -n "$certificates" ] && [ -n "$state" ] && [ -z "$out$shard" ] ||
    { echo "FPTAYLOR_CUT_STATE_OUT builds one problem's cut terms from FPTAYLOR_STATE and FPTAYLOR_CERTIFICATES"; exit 1; }
fi
if [ -n "$state" ]; then
  [ -n "$shard$cut_state_out" ] && [[ $state_sha256 =~ ^[0-9a-f]{64}$ ]] ||
    { echo "FPTAYLOR_STATE is for a shard, with the FPTAYLOR_STATE_SHA256 its making job printed"; exit 1; }
fi
# A saved state holds its folder's paths, so the run that saves it and the shards that restore it
# use the same one, new in each container.
if [ -n "$state_out$state" ]; then
  work=/tmp/fptaylor-hol-state/$(basename "$1" .fpt)
  [ ! -e "$work" ] || { echo "$work is in use"; exit 1; }
  mkdir -p "$work"
else
  work=$(mktemp -d)
fi
# What this run proves, for the session: every term (no shard), or the terms the shard proves
# whole, as an OCaml list, and each term it proves pieces of, as `(term, depth, [pieces]); `. The
# run that saves a cut state proves nothing: it names each cut term with no pieces (shard_setup).
shard_whole=true
shard_setup=false
shard_terms=
shard_cuts=
if [ -n "$shard" ]; then
  [ $# -eq 1 ] && [ -n "$certificates" ] && [ -z "$out" ] ||
    { echo "a shard checks one problem's certificate from FPTAYLOR_CERTIFICATES"; exit 1; }
  shards=$(sed -n 's|^// hol-light-shards: ||p' "$1")
  [[ $shards =~ ^[1-9][0-9]*$ && $shard =~ ^[1-9][0-9]*$ ]] && [ "$shard" -le "$shards" ] ||
    { echo "$1: no shard $shard of '$shards'"; exit 1; }
  shard_count=$(sed -n 's|^// hol-light-terms: ||p' "$1")
  shard_list=$(sed -n "s|^// hol-light-shard $shard: ||p" "$1")
  numbers='^[1-9][0-9]*(\.[1-9][0-9]*)?( [1-9][0-9]*(\.[1-9][0-9]*)?)*$'
  [[ $shard_count =~ ^[1-9][0-9]*$ && $shard_list =~ $numbers ]] ||
    { echo "$1: no terms for shard $shard"; exit 1; }
  shard_whole=false
  # The terms proved whole, and the pieces (t.p), each followed by a space.
  whole=$(tr ' ' '\n' <<<"$shard_list" | grep -v '\.' | tr '\n' ' ' || true)
  pieces=$(tr ' ' '\n' <<<"$shard_list" | grep '\.' | tr '\n' ' ' || true)
  shard_terms=$(echo $whole | sed 's/ /; /g')
  # The shard assumes every term it does not prove whole, less each it proves from its pieces,
  # and the pieces of those it does not prove.
  assume=$((shard_count - $(wc -w <<<"$whole")))
  joins=
  while read -r t d; do
    own=$(tr ' ' '\n' <<<"$pieces" | sed -n "s/^$t\.//p" | tr '\n' ' ')
    for p in $own; do
      [ "$p" -le $((1 << d)) ] || { echo "$1: no piece $t.$p of $((1 << d))"; exit 1; }
    done
    if [ -n "$own" ]; then
      shard_cuts+="($t, $d, [$(echo $own | sed 's/ /; /g')]); "
      assume=$((assume - 1 + (1 << d) - $(wc -w <<<"$own")))
      joins+="$t "
    fi
  done < <(sed -n 's|^// hol-light-cut \([1-9][0-9]*\): \([1-9][0-9]*\)$|\1 \2|p' "$1")
  for p in $pieces; do
    grep -q "^// hol-light-cut ${p%%.*}: " "$1" || { echo "$1: piece $p of a term not cut"; exit 1; }
  done
elif [ -n "$cut_state_out" ]; then
  shard_whole=false
  shard_setup=true
  cut_terms=
  while read -r t d; do
    shard_cuts+="($t, $d, []); "
    cut_terms+="$t "
  done < <(sed -n 's|^// hol-light-cut \([1-9][0-9]*\): \([1-9][0-9]*\)$|\1 \2|p' "$1")
  [ -n "$cut_terms" ] || { echo "$1: no term cut into pieces"; exit 1; }
elif [ -n "$certificates" ]; then
  echo "FPTAYLOR_CERTIFICATES is for a shard"; exit 1
fi

# The image's user installs system packages through sudo; a root user needs none.
SUDO=$(command -v sudo || true)
$SUDO apt-get update -qq
$SUDO env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq git gcc make m4 pkg-config libpcre2-dev ${state_out:+g++} >/dev/null
eval "$(opam env)"
opam update -q >/dev/null
opam install -y num camlp5 camlp-streams ocamlfind >/dev/null
opam list --installed | grep -E '^(ocaml|num|camlp5|camlp-streams|ocamlfind|pcre2|re) '

fetch() {
  git init -q "$2"
  git -C "$2" fetch -q --depth 1 "$1" "$3"
  git -C "$2" checkout -q FETCH_HEAD
}
fetch https://github.com/soarlab/FPTaylor.git "$work/FPTaylor" "$FPTAYLOR_COMMIT"
fetch https://github.com/jrh13/hol-light.git "$work/hol-light" "$HOL_LIGHT_COMMIT"

# HOL Light's inequality verifier precomputes its multivariate Taylor theorems for up to
# `max_dim` = 8 variables (Formal_ineqs/taylor/m_taylor.hl), and orient3d's filter has nine. The
# constant only sizes those tables (each built for 1 to max_dim variables), so it is raised to the
# most variables any problem has: its inputs and its ratio parameters (piece G40).
most=8
for problem in "$@"; do
  n=$(grep -c '^ *\(float64\|real\) .* in \[.*\];' "$problem" || true)
  [ "$n" -gt "$most" ] && most=$n
done
sed -i "s/^let max_dim = 8;;$/let max_dim = $most;;/" "$work/hol-light/Formal_ineqs/taylor/m_taylor.hl"
grep -n '^let max_dim' "$work/hol-light/Formal_ineqs/taylor/m_taylor.hl"

# The verifier builds a function's Taylor theorems on each call, which for a large term takes
# longer than checking a piece of its box; fptaylor_cache.hl has it build them once a function,
# so a cut term's pieces share them.
verifier="$work/hol-light/Formal_ineqs/verifier/m_verifier_main.hl"
sed -i 's/^let mk_verification_functions =$/let mk_verification_functions_uncached =/' "$verifier"
awk -v f="$here/fptaylor_cache.hl" \
  '/^\(\* Normalizes a verification result \*\)$/ { while ((getline l < f) > 0) print l } { print }' \
  "$verifier" >"$verifier.new"
mv "$verifier.new" "$verifier"
[ "$(grep -c '^let mk_verification_functions_uncached =$' "$verifier")" = 1 ] &&
  [ "$(grep -c '^let mk_verification_functions =$' "$verifier")" = 1 ] ||
  { echo "$verifier: no mk_verification_functions to remember"; exit 1; }

# FPTaylor's checker proves each term's inequality in turn (formal/build_tform2.hl,
# prove_opt_approx). Here each goes through shard_prove (fptaylor_cut.hl, put in after
# prove_bound_cond): proved whole, naming each, when this shard names it or there is no shard;
# proved from its pieces when this shard names some of them; assumed otherwise, so the theorem
# the loop ends with holds under the terms and pieces assumed.
checker="$work/FPTaylor/formal/build_tform2.hl"
sed -i 's|^\(\s*\)prove_bound_cond pp tm) bound_conds in$|\1shard_prove pp !counter total_bounds tm) bound_conds in|' "$checker"
sed -i "/^  with _ -> error \"prove_bound_cond\" \[tm\] \[\];;\$/r $here/fptaylor_cut.hl" "$checker"
[ "$(grep -c 'shard_prove pp !counter total_bounds tm) bound_conds in' "$checker")" = 1 ] &&
  [ "$(grep -c '^let shard_prove ' "$checker")" = 1 ] ||
  { echo "$checker: no term loop to shard"; exit 1; }

# The first part of the HOL Light session, the same for every problem with as many variables: load
# HOL Light, its multivariate analysis and the inequality verifier bundled with it (Formal_ineqs),
# with this machine's paths. A saved state is this part, loaded.
load_session() {
  # camlp5 8.03.06 and later need libraries of their own (PCRE2, Re, camlp-streams' Stream), which
  # HOL Light's loader for OCaml before 4.14 does not load: it loads camlp5o.cma directly. So
  # findlib loads camlp5 with everything it needs first, as HOL Light's own loader for OCaml 4.14
  # does (load_camlp5_topfind.ml), and hol.ml then loads camlp5o.cma. camlp5's findlib package
  # does not name camlp-streams, whose `Stream` camlp5o.cma needs, so it is loaded by name.
  echo '#use "topfind";;'
  echo 'Topfind.don'"'"'t_load ["compiler-libs.common"];;'
  echo 'Topfind.load_deeply ["camlp-streams"; "pcre2"; "re"; "camlp5"];;'
  echo "#use \"$work/hol-light/hol.ml\";;"
  echo "let formal_ineqs_path = \"$work/hol-light/Formal_ineqs\";;"
  echo "let fptaylor_path = \"$work/FPTaylor\";;"
  echo 'let fptaylor_formal_path = fptaylor_path ^ "/formal";;'
  echo 'load_path := formal_ineqs_path :: fptaylor_formal_path :: !load_path;;'
  echo 'needs "arith_options.hl";;'
  echo 'Arith_options.base := 100;;'
  echo 'Arith_options.min_exp := 300;;'
  echo 'needs "Multivariate/realanalysis.ml";;'
  echo 'needs "verifier/m_verifier_main.hl";;'
}

# A saved state is restored on whichever runner a shard gets. glibc picks each string and memory
# routine for the processor it starts on, and a restored process keeps those picks, so HOL Light
# starts with glibc's AVX-512 and TSX routines turned off (glibc's hwcaps tunable): every runner
# has the rest of what glibc can pick (`cpu_flags`), and a shard checks it has each flag of the
# runner that saved the state.
tunables=glibc.cpu.hwcaps=-AVX512F,-AVX512CD,-AVX512BW,-AVX512DQ,-AVX512VL,-RTM
cpu_flags() {
  local all
  all=" $(sed -n 's/^flags\s*: //p' /proc/cpuinfo | head -n 1) "
  for flag in sse4_1 sse4_2 ssse3 popcnt abm bmi1 bmi2 movbe avx avx2 fma; do
    [[ $all != *" $flag "* ]] || printf '%s ' "$flag"
  done
}

# Saves the HOL Light process `hol_pid`, which runs under the DMTCP coordinator whose port is in
# $work/port and reads the pipe open as descriptor 3, with $work/manifest, to
# <dir>/hol-light-state.tar, and prints the file's sha256.
save_state() {
  local port images
  port=$(cat "$work/port")
  # v4.2.0's coordinator starts the checkpoint `-bc` asks for but does not wait for it (its log:
  # "unhandled user command; (cmd = b)"), so, as HOL Light's make-checkpoint.sh does, the state is
  # taken as written once the coordinator has written its restart script, which it does after
  # every image; only then is HOL Light stopped.
  "$work/dmtcp/bin/dmtcp_command" --coord-port "$port" -bc
  for _ in $(seq 360); do
    [ ! -e "$work/checkpoint/dmtcp_restart_script.sh" ] || break
    kill -0 "$hol_pid" 2>/dev/null || break
    sleep 5
  done
  [ -e "$work/checkpoint/dmtcp_restart_script.sh" ] ||
    { echo "DMTCP wrote no restart script: the state was not saved"; exit 1; }
  "$work/dmtcp/bin/dmtcp_command" --coord-port "$port" --quit
  exec 3>&-
  wait "$hol_pid" || true
  wait
  images=("$work"/checkpoint/ckpt_*.dmtcp)
  [ ${#images[@]} -eq 1 ] && [ -f "${images[0]}" ] || { echo "DMTCP saved no single image"; exit 1; }
  mv "${images[0]}" "$work/hol.dmtcp"
  cat "$work/manifest"
  echo "saved on: $(sed -n 's/^model name\s*: //p' /proc/cpuinfo | head -n 1)"
  tar -C "$work" -cf "$1/hol-light-state.tar" hol.dmtcp dmtcp manifest
  ls -l "$1/hol-light-state.tar"
  echo "HOL Light state sha256 $(sha256sum <"$1/hol-light-state.tar" | cut -d' ' -f1)"
}

if [ -n "$state_out" ]; then
  echo "::group::Build HOL Light and DMTCP"
  make -C "$work/hol-light" >/dev/null
  fetch https://github.com/dmtcp/dmtcp.git "$work/dmtcp-source" "$DMTCP_COMMIT"
  (cd "$work/dmtcp-source" && ./configure --prefix="$work/dmtcp" >/dev/null && make -j "$(nproc)" >/dev/null &&
    make install >/dev/null)
  echo "::endgroup::"
  load_session >"$work/load.ml"
  echo 'print_endline "HOL Light state: loaded";;' >>"$work/load.ml"
  cat "$work/load.ml"
  # HOL Light reads the session from a pipe held open until the state is saved: at the end of its
  # input it would stop.
  mkfifo "$work/input"
  mkdir "$work/checkpoint"
  export HOLLIGHT_DIR="$work/hol-light"
  GLIBC_TUNABLES=$tunables "$work/dmtcp/bin/dmtcp_launch" --new-coordinator --coord-port 0 \
    --port-file "$work/port" --ckptdir "$work/checkpoint" --no-gzip \
    "$work/hol-light/ocaml-hol" -I "$(camlp5 -where)" -safe-string -I "$work/hol-light" \
    <"$work/input" >"$work/hol.log" 2>&1 &
  hol_pid=$!
  exec 3>"$work/input"
  cat "$work/load.ml" >&3
  tail -n +1 -f --pid="$hol_pid" "$work/hol.log" | grep --line-buffered -v '^val \|^- : ' &
  until grep -q '^\(# \)*HOL Light state: loaded$' "$work/hol.log"; do
    kill -0 "$hol_pid" 2>/dev/null || { echo "HOL Light stopped before it was loaded"; exit 1; }
    sleep 5
  done
  {
    echo "hol-light $HOL_LIGHT_COMMIT"
    echo "dmtcp $DMTCP_COMMIT"
    echo "max_dim $most"
    echo "cpu-flags $(cpu_flags)"
  } >"$work/manifest"
  save_state "$state_out"
  exit 0
fi

if [ -z "$certificates" ]; then
  echo "::group::Build FPTaylor"
  make -C "$work/FPTaylor" >/dev/null
  echo "::endgroup::"
fi

# Each problem runs in a folder of its own: FPTaylor names a certificate after its expression, and
# two problems may share one (`det`).
checks=()
for problem in "$@"; do
  name=$(basename "$problem" .fpt)
  run="$work/run/$name"
  mkdir -p "$run"
  cp "$here/fptaylor.cfg" "$run/"
  expression=$(sed -n '/^Expressions/,$ s/^ *\([a-z_0-9]*\) *=.*/\1/p' "$problem" | head -n 1)
  bound=$(sed -n 's|^// hol-light-bound: ||p' "$problem")
  [ -n "$expression" ] && [ -n "$bound" ] || { echo "$problem: no expression or bound"; exit 1; }
  total=$(sed -n 's|^// hol-light-total: ||p' "$problem")
  case $(sed -n 's|^// fptaylor-optimisation: ||p' "$problem") in
    approx) printf 'opt-exact = false\nopt-approx = true\n' >>"$run/fptaylor.cfg" ;;
    exact) printf 'opt-exact = true\nopt-approx = false\n' >>"$run/fptaylor.cfg" ;;
    *) echo "$problem: no optimisation"; exit 1 ;;
  esac
  # The problem's absolute path, found before FPTaylor runs in its own folder.
  path="$(cd "$(dirname "$problem")" && pwd)/$(basename "$problem")"
  if [ -n "$certificates" ]; then
    mkdir -p "$run/proofs"
    cp "$certificates/$name.proof" "$run/proofs/$expression.proof"
  else
    echo "::group::FPTaylor on $name"
    (cd "$run" && "$work/FPTaylor/fptaylor" -c fptaylor.cfg "$path")
    echo "::endgroup::"
  fi
  if [ -n "$out" ]; then
    cp "$run/proofs/$expression.proof" "$out/$name.proof"
  fi
  echo "$name: certificate sha256 $(sha256sum <"$run/proofs/$expression.proof" | cut -d' ' -f1)"
  checks+=("$run/proofs|$expression|$bound|$name|$total")
done
[ -z "$out" ] || exit 0

session="$work/session.ml"
if [ -n "$state" ]; then
  # The saved state, only if it is the file its making job printed the digest of, made from the
  # commits and Taylor tables this run would load, on a processor whose flags this one has.
  echo "::group::Restore HOL Light's saved state"
  digest=$(sha256sum <"$state/hol-light-state.tar" | cut -d' ' -f1)
  [ "$digest" = "$state_sha256" ] ||
    { echo "the saved state's sha256 is $digest, not $state_sha256, which its making job printed"; exit 1; }
  echo "saved state sha256 $digest, as its making job printed"
  tar -C "$work" -xf "$state/hol-light-state.tar"
  cat "$work/manifest"
  for want in "hol-light $HOL_LIGHT_COMMIT" "dmtcp $DMTCP_COMMIT" "max_dim $most"; do
    grep -qx "$want" "$work/manifest" || { echo "the saved state was not made with $want"; exit 1; }
  done
  here_flags=" $(cpu_flags)"
  for flag in $(sed -n 's/^cpu-flags //p' "$work/manifest"); do
    [[ $here_flags == *" $flag "* ]] ||
      { echo "the saved state was made on a processor with $flag, and this one has none"; exit 1; }
  done
  echo "restored on: $(sed -n 's/^model name\s*: //p' /proc/cpuinfo | head -n 1)"
  echo "::endgroup::"
  hol=("$work/dmtcp/bin/dmtcp_restart" --new-coordinator --coord-port 0 "$work/hol.dmtcp")
  echo 'print_endline "HOL Light state: restored";;' >"$session"
else
  echo "::group::Build HOL Light"
  make -C "$work/hol-light" >/dev/null
  echo "::endgroup::"
  export HOLLIGHT_DIR="$work/hol-light"
  hol=("$work/hol-light/ocaml-hol" -I "$(camlp5 -where)" -safe-string -I "$work/hol-light")
  load_session >"$session"
fi

# The rest of the HOL Light session: FPTaylor's checker (FPTaylor's formal/load.hl, with this
# machine's paths), then each certificate checked against its bound.
{
  echo 'needs "build_tform2.hl";;'
  echo 'needs "tests/utils.hl";;'
  # What this run proves (fptaylor_cut.hl), set once the checker is loaded: a cut state holds the
  # checker loaded, and the two lines above then load nothing.
  echo "Build_tform2.shard_whole := $shard_whole;;"
  echo "Build_tform2.shard_setup := $shard_setup;;"
  echo "Build_tform2.shard_terms := [$shard_terms];;"
  echo "Build_tform2.shard_cuts := [$shard_cuts];;"
  # A shard's theorems hold under the terms it assumed: their conclusions are printed, with how
  # many terms they assume.
  echo 'let print_bound th = if hyp th = [] then print_thm th else (print_term (concl th); Printf.printf "\n(assuming %d terms)" (length (hyp th)));;'
  for check in "${checks[@]}"; do
    IFS='|' read -r proofs expression bound name total <<<"$check"
    echo "print_endline \"PROBLEM: $name\";;"
    echo "path := \"$proofs\";;"
    if [ -n "$total" ]; then
      echo "let _, bound_th, time = let k = float_of_string \"$total\" in if (hd (load \"$expression.proof\").proof_opts).total_bound <= k then validate_gen 10 \"$expression.proof\" k else validate 10 \"$expression.proof\";;"
    else
      echo "let _, bound_th, time = validate 10 \"$expression.proof\";;"
    fi
    echo 'print_endline "CERTIFICATE:";; print_bound bound_th;; print_newline ();;'
    echo "Printf.printf \"checked $name in %.0f s\\n\" time;;"
    echo "Printf.printf \"$name assumed %d terms\\n\" (length (hyp bound_th));;"
    echo "(try let th = trans_bound bound_th \`$bound\` in print_bound th; print_newline (); print_endline \"$name: within K\" with Failure _ -> print_endline \"$name: above K\");;"
  done
} >>"$session"

if [ -n "$cut_state_out" ]; then
  # The restored HOL Light reads the session from a pipe held open until the state is saved again,
  # as the run that saved the loaded state does. The coordinator of a restored process writes its
  # restart script where it runs, so it runs in the folder the image goes to.
  echo 'print_endline "HOL Light state: cut terms built";;' >>"$session"
  cat "$session"
  mkfifo "$work/input"
  mkdir "$work/checkpoint"
  (cd "$work/checkpoint" && exec "$work/dmtcp/bin/dmtcp_restart" --new-coordinator --coord-port 0 \
    --port-file "$work/port" --ckptdir "$work/checkpoint" "$work/hol.dmtcp") \
    <"$work/input" >"$work/hol.log" 2>&1 &
  hol_pid=$!
  exec 3>"$work/input"
  cat "$session" >&3
  tail -n +1 -f --pid="$hol_pid" "$work/hol.log" | grep --line-buffered -v '^val \|^- : ' &
  until grep -q '^\(# \)*HOL Light state: cut terms built$' "$work/hol.log"; do
    kill -0 "$hol_pid" 2>/dev/null || { echo "HOL Light stopped before the cut terms were built"; exit 1; }
    sleep 5
  done
  # Each cut term's theorems were built here, and nothing was proved: every term is assumed.
  built=$(sed -n 's|^\(# \)*Built: \([0-9][0-9]*\)/[0-9]* Taylor theorems in [0-9]* s$|\2|p' "$work/hol.log" | tr '\n' ' ')
  [ "$built" = "$cut_terms" ] || { echo "built the Taylor theorems of terms '$built', not '$cut_terms'"; exit 1; }
  ! grep -q '^\(# \)*Proved: ' "$work/hol.log" || { echo "the run that saves a cut state proved a term"; exit 1; }
  {
    echo "cpu-flags $(cpu_flags)"
    echo "cut-terms $cut_terms"
  } >>"$work/manifest"
  save_state "$cut_state_out"
  exit 0
fi
cat "$session"

"${hol[@]}" <"$session" | tee "$work/hol.log" | grep --line-buffered -v '^val \|^- : ' || true
# Every problem's certificate must have checked; whether each bound is within K is reported. The
# toplevel may print its prompt, "# ", before a line.
for check in "${checks[@]}"; do
  IFS='|' read -r _ _ _ name _ <<<"$check"
  grep -q "^\(# \)*checked $name " "$work/hol.log" || { echo "HOL Light did not check $name"; exit 1; }
  grep -h "^\(# \)*$name: \(within\|above\) K$" "$work/hol.log" || echo "$name: K not compared"
  # Every term is proven, or for a shard exactly its own whole terms and pieces, each cut term it
  # has pieces of joined from them, the rest assumed. The exact problem is one inequality, and
  # names no terms.
  awk -v n="$name" '/^(# )*PROBLEM: / { on = ($NF == n) } on' "$work/hol.log" >"$work/$name.log"
  terms=$(sed -n 's/^\(# \)*Bounds: \([0-9][0-9]*\)$/\2/p' "$work/$name.log")
  [ -n "$terms" ] || terms=0
  proved=$(sed -n 's|^\(# \)*Proved: \([0-9][0-9]*\)/[0-9]*$|\2|p' "$work/$name.log" | tr '\n' ' ')
  proved_pieces=$(sed -n 's|^\(# \)*Proved: \([0-9][0-9]*\)/[0-9]* piece \([0-9][0-9]*\)/[0-9]* in [0-9]* s$|\2.\3|p' "$work/$name.log" | tr '\n' ' ')
  joined=$(sed -n 's|^\(# \)*Joined: \([0-9][0-9]*\)/[0-9]* from [0-9]* pieces, [0-9]* of them proved here$|\2|p' "$work/$name.log" | tr '\n' ' ')
  if [ -n "$shard" ]; then
    [ "$terms" = "$shard_count" ] ||
      { echo "$name: the certificate has $terms terms, the shards share out $shard_count"; exit 1; }
    want=$whole
  else
    want=$(if [ "$terms" -gt 0 ]; then seq 1 "$terms" | tr '\n' ' '; fi)
    pieces= joins= assume=0
  fi
  assumed=$(sed -n "s/^\\(# \\)*$name assumed \\([0-9][0-9]*\\) terms$/\\2/p" "$work/$name.log")
  if [ "$proved" != "$want" ] || [ "$proved_pieces" != "$pieces" ] || [ "$joined" != "$joins" ] ||
    [ "$assumed" != "$assume" ]; then
    echo "$name: proved terms '$proved', pieces '$proved_pieces', joined '$joined' and assumed '$assumed' of $terms, not shard ${shard:-1} of $shards"
    exit 1
  fi
  [ -z "$shard" ] || echo "$name: shard $shard of $shards proved terms '${want% }' and pieces '${pieces% }' of $terms and assumed the other $assumed"
done
