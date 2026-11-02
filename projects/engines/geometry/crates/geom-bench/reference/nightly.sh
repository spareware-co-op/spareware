#!/usr/bin/env bash
# The reference runner's night (piece G107; chapter 23.13 of the kernel architecture, "Wall clock
# and scaling", and 28.17's reference machine, decided in 32.16).
#
# Times every benchmark of geom-bench's catalogue at 1 and 8 threads, natively and in Chrome,
# Firefox and WebKit, on the reference machine's performance cores, records the night, rewrites the
# report against 23.1's shares and, with --push, adds both to the repository's `kernel-timings`
# branch. It reports and never blocks: G452 turns the timings into a gate.
#
# Runs in WSL 2 on the reference machine (Ubuntu 24.04 on Windows 11), in a clean checkout of the
# commit to time; the build workstation starts it over SSH each night, after 19:00 New Zealand
# time, for example:
#
#   git -C <checkout> fetch -q origin && git -C <checkout> checkout -q --detach origin/main &&
#     <checkout>/projects/engines/geometry/crates/geom-bench/reference/nightly.sh \
#       --windows-dir /mnt/<drive>/<folder> --push
#
# Options:
#   --windows-dir <dir>  a folder on a Windows drive, seen from WSL (/mnt/<drive>/...), for what
#                        runs on Windows: the guest, Node's packages and Playwright's browsers.
#   --work <dir>         the runner's own folder in WSL (default ~/.cache/spare-kit-reference):
#                        its lock, tools, the night's files and the history.
#   --round <ms>         each of the guest's rounds (default 150); six rounds per benchmark.
#   --cases <regex>      time only the benchmarks whose names match (default: every one). A night
#                        run this way is for testing the runner: --push refuses it.
#   --engines <list>     the engines to run, comma-separated (default native,chrome,firefox,webkit).
#                        Like --cases, for testing; --push refuses it.
#   --any-time           run outside 19:00 to 08:00 (New Zealand time), for testing; the 08:00
#                        deadline still holds.
#   --push               commit the record and the report to `kernel-timings` and push it.
#
# Why it is built this way:
#
# - Timings come from the native Windows build, pinned with a processor affinity mask (decision
#   32.16): WSL 2's virtual processors are not tied to host cores, so a run in WSL could land half on
#   the efficient cores. The guest is cross-built in WSL for x86_64-pc-windows-gnullvm with
#   llvm-mingw (Apache-2.0 WITH LLVM-exception; its MinGW-w64 runtime under the Zope Public
#   Licence and public-domain terms), pinned below and checked by SHA-256, and linked statically.
# - 1 thread runs on the first performance core, 8 threads on all eight (machine.ps1 finds them by
#   Windows' efficiency classes). A pinned job is started by a two-line batch file (pin.cmd, written
#   here): `start /affinity` sets the mask as the process starts, and `exit /b %errorlevel%`
#   returns the guest's own exit status.
# - The browsers run on Windows too, through Node and Playwright, pinned to the eight performance
#   cores; each worker thread lands on one of them.
# - The machine is lent for the night only (19:00 to 08:00): no job starts unless it can end by
#   08:00, the browsers stop there, and the busy sampler stops there.
# - busy.ps1 samples how busy the whole machine is each second, so a timing taken while something
#   else ran is marked, never silently trusted (geom_bench::timings, "disturbed").

set -euo pipefail

# Pinned tools: llvm-mingw's release and its SHA-256 (GitHub's release digest), and the npm
# packages, the same versions the variant workflow pins (kernel-variants.yml).
LLVM_MINGW=llvm-mingw-20261020-ucrt-ubuntu-22.04-x86_64
LLVM_MINGW_SHA256=55064b60b991a1e1e8090b8c2f84e82b22cfe5903d12c159d65064cf5e8ae335
LLVM_MINGW_URL=https://github.com/mstorsjo/llvm-mingw/releases/download/20261020/$LLVM_MINGW.tar.xz
PLAYWRIGHT=1.52.0
WASI_SHIM=0.3.0
WINDOWS_TARGET=x86_64-pc-windows-gnullvm
ENGINES_ALL=native,chrome,firefox,webkit

windows_dir=""
work="$HOME/.cache/spare-kit-reference"
round=150
cases=""
engines="$ENGINES_ALL"
any_time=0
push=0
while [ $# -gt 0 ]; do
  case "$1" in
    --windows-dir) windows_dir="$2"; shift 2 ;;
    --work) work="$2"; shift 2 ;;
    --round) round="$2"; shift 2 ;;
    --cases) cases="$2"; shift 2 ;;
    --engines) engines="$2"; shift 2 ;;
    --any-time) any_time=1; shift ;;
    --push) push=1; shift ;;
    *) echo "nightly.sh: unknown option $1" >&2; exit 2 ;;
  esac
done
say() { echo "nightly.sh: $*" >&2; }
fail() { say "$*"; exit 1; }

[ -n "$windows_dir" ] || fail "--windows-dir is required"
case "$windows_dir" in /mnt/?/*) ;; *) fail "--windows-dir must be on a Windows drive, /mnt/<drive>/..." ;; esac
[[ "$round" =~ ^[0-9]+$ ]] && [ "$round" -ge 1 ] && [ "$round" -le 60000 ] || fail "--round must be 1 to 60000"
if [ "$push" = 1 ] && { [ -n "$cases" ] || [ "$engines" != "$ENGINES_ALL" ]; }; then
  fail "--push records whole nights only: drop --cases and --engines"
fi
for e in ${engines//,/ }; do
  case ",$ENGINES_ALL," in *",$e,"*) ;; *) fail "unknown engine $e" ;; esac
done

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(git -C "$here" rev-parse --show-toplevel)"
[ -z "$(git -C "$repo" status --porcelain --untracked-files=no)" ] || fail "the checkout has changes: time a clean commit"
commit="$(git -C "$repo" rev-parse HEAD)"

# The night: it begins at 19:00 and ends at 08:00 the next morning, New Zealand time (28.17).
nz() { TZ=Pacific/Auckland date "$@"; }
hour=$((10#$(nz +%H)))
if [ "$hour" -ge 8 ]; then
  night="$(nz +%F)"
  deadline_s="$(nz -d 'tomorrow 08:00' +%s)"
else
  night="$(nz -d yesterday +%F)"
  deadline_s="$(nz -d 'today 08:00' +%s)"
fi
if [ "$any_time" = 0 ] && [ "$hour" -lt 19 ] && [ "$hour" -ge 8 ]; then
  fail "the reference machine is lent from 19:00 to 08:00 (New Zealand time); it is $(nz +%H:%M)"
fi
deadline_ms=$((deadline_s * 1000))
now_ms() { date +%s%3N; }
# A job may start only if it can end by the deadline: six rounds, a margin of two more, and five
# seconds to start the process and build and check its batches.
fits() { [ $(( $(now_ms) + 8 * round + 5000 )) -le "$deadline_ms" ]; }

mkdir -p "$work" "$windows_dir"
exec 9> "$work/lock"
flock -n 9 || fail "another night is running ($work/lock)"
tonight="$work/night"
rm -rf "$tonight"
mkdir -p "$tonight"

# --- Tools -------------------------------------------------------------------------------------
mkdir -p "$work/tools"
if [ ! -x "$work/tools/$LLVM_MINGW/bin/x86_64-w64-mingw32-clang" ]; then
  say "fetching $LLVM_MINGW"
  curl --fail --silent --show-error --location --retry 3 --output "$work/tools/$LLVM_MINGW.tar.xz" "$LLVM_MINGW_URL"
  echo "$LLVM_MINGW_SHA256  $work/tools/$LLVM_MINGW.tar.xz" | sha256sum --check --strict --quiet
  tar -C "$work/tools" -xf "$work/tools/$LLVM_MINGW.tar.xz"
  rm -f "$work/tools/$LLVM_MINGW.tar.xz"
fi
(cd "$repo" && rustup target add "$WINDOWS_TARGET" wasm32-wasip1 > /dev/null)
toolchain="$(cd "$repo" && rustc --version)"

# --- Builds (nothing is timed while building) --------------------------------------------------
export CARGO_INCREMENTAL=0
export "CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER=$work/tools/$LLVM_MINGW/bin/x86_64-w64-mingw32-clang"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_RUSTFLAGS="-C target-feature=+crt-static"
build() { (cd "$repo" && cargo build --release --locked --quiet -p geom-bench "$@"); }
say "building geom-bench-guest for Windows and WebAssembly, and geom-bench-timings"
build --bin geom-bench-guest --target "$WINDOWS_TARGET"
build --bin geom-bench-guest --target wasm32-wasip1
build --bin geom-bench-timings
target="$(cd "$repo" && cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
timings="$target/release/geom-bench-timings"
cp "$target/$WINDOWS_TARGET/release/geom-bench-guest.exe" "$windows_dir/guest.exe"
cp "$target/wasm32-wasip1/release/geom-bench-guest.wasm" "$windows_dir/guest.wasm"
cp "$here/machine.ps1" "$here/busy.ps1" "$here/browsers.mjs" "$here/page.html" "$here/page.mjs" "$here/worker.mjs" "$windows_dir/"
printf '@echo off\r\nstart "" /b /wait /affinity %%1 %%2 %%3 %%4 %%5 %%6 %%7 %%8 %%9\r\nexit /b %%errorlevel%%\r\n' > "$windows_dir/pin.cmd"

# Everything Windows runs is started from the Windows folder, with nothing read from standard input.
win() { (cd "$windows_dir" && "$@" < /dev/null); }
winpath() { wslpath -w "$1"; }

# --- The machine -------------------------------------------------------------------------------
win powershell.exe -NoProfile -ExecutionPolicy Bypass -File machine.ps1 | tr -d '\r' > "$tonight/machine.txt"
performance="$(sed -n 's/^performance //p' "$tonight/machine.txt")"
read -r -a cores <<< "$performance"
[ "${#cores[@]}" -ge 8 ] || fail "8 performance cores are needed (32.16); found ${#cores[@]}: $performance"
mask_of() { local m=0 c; for c in "$@"; do m=$((m | (1 << c))); done; printf '%X' "$m"; }
mask1="$(mask_of "${cores[0]}")"
mask8="$(mask_of "${cores[@]:0:8}")"
say "performance cores ${cores[*]}; 1 thread on mask $mask1, 8 threads on mask $mask8"

# --- The benchmarks ----------------------------------------------------------------------------
win ./guest.exe list | tr -d '\r' > "$tonight/all.txt"
if [ -n "$cases" ]; then grep -E "$cases" "$tonight/all.txt" > "$tonight/cases.txt" || true; else cp "$tonight/all.txt" "$tonight/cases.txt"; fi
[ -s "$tonight/cases.txt" ] || fail "no benchmark matches --cases $cases"
cp "$tonight/cases.txt" "$windows_dir/cases.txt"
say "$(wc -l < "$tonight/cases.txt") benchmarks, deadline $(nz -d "@$deadline_s" '+%F %H:%M %Z')"

# --- The busy sampler, for the whole night -----------------------------------------------------
rm -f "$windows_dir/busy.txt" "$windows_dir/busy.stop"
win powershell.exe -NoProfile -ExecutionPolicy Bypass -File busy.ps1 \
  -Output "$(winpath "$windows_dir")\\busy.txt" -Stop "$(winpath "$windows_dir")\\busy.stop" \
  -Deadline "$deadline_ms" > /dev/null 2>&1 &
sampler=$!
stop_sampler() { touch "$windows_dir/busy.stop"; wait "$sampler" 2> /dev/null || true; }
trap stop_sampler EXIT

status=complete
started="$(now_ms)"
: > "$tonight/jobs.txt"
declare -A version

# --- Native ------------------------------------------------------------------------------------
if [[ ",$engines," == *,native,* ]]; then
  version[native]="$WINDOWS_TARGET, llvm-mingw ${LLVM_MINGW#llvm-mingw-}, statically linked"
  for threads in 1 8; do
    if [ "$threads" = 1 ]; then mask="$mask1"; else mask="$mask8"; fi
    while read -r name; do
      if ! fits; then status=deadline; break 2; fi
      start="$(now_ms)"
      set +e
      win cmd.exe /c ".\\pin.cmd $mask .\\guest.exe time $name $round $threads" > "$tonight/out.txt" 2> "$tonight/err.txt"
      code=$?
      set -e
      end="$(now_ms)"
      if [ "$code" = 0 ]; then
        text="$(tr -d '\r' < "$tonight/out.txt" | head -1)"
      else
        text="$name $(tr -d '\r' < "$tonight/err.txt" | { grep -v '^$' || true; } | head -1)"
      fi
      echo "job native $threads $start $end $code $text" >> "$tonight/jobs.txt"
    done < "$tonight/cases.txt"
  done
fi

# --- Browsers ----------------------------------------------------------------------------------
browsers=()
for e in chrome firefox webkit; do [[ ",$engines," == *,$e,* ]] && browsers+=("$e"); done
if [ "${#browsers[@]}" -gt 0 ] && [ "$status" = complete ]; then
  # Node's packages and Playwright's browsers live in the Windows folder; npm's cache too.
  export WSLENV="${WSLENV:+$WSLENV:}npm_config_cache/w:PLAYWRIGHT_BROWSERS_PATH/w"
  export npm_config_cache="$(winpath "$windows_dir")\\npm-cache"
  export PLAYWRIGHT_BROWSERS_PATH="$(winpath "$windows_dir")\\browsers"
  installed() { python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$windows_dir/node_modules/$1/package.json" 2> /dev/null || true; }
  if [ "$(installed playwright)" != "$PLAYWRIGHT" ] || [ "$(installed @bjorn3/browser_wasi_shim)" != "$WASI_SHIM" ]; then
    say "installing playwright@$PLAYWRIGHT and @bjorn3/browser_wasi_shim@$WASI_SHIM"
    win cmd.exe /c "npm install --no-save --no-audit --no-fund playwright@$PLAYWRIGHT @bjorn3/browser_wasi_shim@$WASI_SHIM" > /dev/null
  fi
  win cmd.exe /c "npx playwright install firefox webkit" > /dev/null
  # Every job a browser did not report, at each thread count, as a failure saying why.
  fill_missing() {
    local e="$1" why="$2" threads name at
    at="$(now_ms)"
    for threads in 1 8; do
      awk -v e="$e" -v t="$threads" '$1 == "job" && $2 == e && $3 == t { print $7 }' "$tonight/jobs.txt" \
        | sort > "$tonight/done.txt"
      sort "$tonight/cases.txt" | comm -23 - "$tonight/done.txt" | while read -r name; do
        echo "job $e $threads $at $at 5 $name $why" >> "$tonight/jobs.txt"
      done
    done
  }
  for e in "${browsers[@]}"; do
    if ! fits; then status=deadline; break; fi
    rm -f "$windows_dir/jobs-$e.txt"
    say "timing in $e"
    set +e
    win cmd.exe /c ".\\pin.cmd $mask8 node browsers.mjs $e $round $deadline_ms cases.txt jobs-$e.txt" 2> "$tonight/$e.err" | tr -d '\r' >&2
    set -e
    why="$(tr -d '\r' < "$tonight/$e.err" | { grep -v '^$' || true; } | head -1)"
    first="$(head -1 "$windows_dir/jobs-$e.txt" 2> /dev/null | tr -d '\r')"
    case "$first" in
      "# version "*)
        version[$e]="${first#\# version }"
        grep '^job ' "$windows_dir/jobs-$e.txt" | tr -d '\r' >> "$tonight/jobs.txt" || true
        if [ "$(grep -c "^job $e " "$tonight/jobs.txt" || true)" -lt $((2 * $(wc -l < "$tonight/cases.txt"))) ]; then
          if ! fits; then status=deadline; break; fi
          fill_missing "$e" "the browser stopped early: ${why:-no message}"
        fi
        ;;
      "# unavailable "*) version[$e]="unavailable: ${first#\# unavailable }" ;;
      *) version[$e]="unavailable: ${why:-the browser printed nothing}" ;;
    esac
  done
fi
finished="$(now_ms)"
stop_sampler
trap - EXIT
tr -d '\r' < "$windows_dir/busy.txt" > "$tonight/busy.txt" 2> /dev/null || : > "$tonight/busy.txt"

# --- The record --------------------------------------------------------------------------------
{
  echo "spare-kit reference timings 1"
  echo "commit $commit"
  echo "night $night"
  echo "started $started"
  echo "finished $finished"
  echo "status $status"
  sed -n 's/^machine /machine /p' "$tonight/machine.txt"
  sed -n 's/^processors /processors /p' "$tonight/machine.txt"
  echo "performance $performance"
  sed -n 's/^os /os /p' "$tonight/machine.txt"
  echo "toolchain $toolchain"
  echo "round $round"
  for e in native chrome firefox webkit; do
    if [[ ",$engines," == *,$e,* ]]; then
      echo "engine $e ${version[$e]:-unavailable: not reached before the deadline}"
    fi
  done
} > "$tonight/header.txt"
# A test night times only some benchmarks: it is judged against the benchmarks it timed.
names="$tonight/all.txt"
if [ -n "$cases" ]; then names="$tonight/cases.txt"; fi
"$timings" assemble "$tonight/header.txt" "$tonight/busy.txt" "$tonight/jobs.txt" "$names" > "$tonight/record.txt"

# --- The history and the report ----------------------------------------------------------------
history="$work/history"
if [ "$push" = 1 ]; then
  branch=kernel-timings
  history="$work/timings/history"
  git -C "$repo" fetch -q origin
  if [ ! -d "$work/timings/.git" ] && [ ! -f "$work/timings/.git" ]; then
    rm -rf "$work/timings"
    if git -C "$repo" rev-parse -q --verify "origin/$branch" > /dev/null; then
      git -C "$repo" worktree add -q --detach "$work/timings" "origin/$branch"
    else
      git -C "$repo" worktree add -q --detach "$work/timings" HEAD
      git -C "$work/timings" checkout -q --orphan "$branch"
      git -C "$work/timings" rm -rq --cached .
      git -C "$work/timings" clean -fdxq
    fi
  elif git -C "$repo" rev-parse -q --verify "origin/$branch" > /dev/null; then
    git -C "$work/timings" checkout -q --detach "origin/$branch"
  fi
fi
mkdir -p "$history"
run=1
while [ -e "$history/$night-$run.txt" ]; do run=$((run + 1)); done
cp "$tonight/record.txt" "$history/$night-$run.txt"
"$timings" check "$history/$night-$run.txt" >&2
"$timings" report "$history" > "$history/../report.md"
say "recorded $history/$night-$run.txt; report in $(dirname "$history")/report.md"

if [ "$push" = 1 ]; then
  cat > "$work/timings/README.md" <<'README'
# Spare Kit reference timings

The geometry kernel's nightly wall-clock timings on the reference machine (piece G107; chapter
23.13 of the kernel architecture, "Wall clock and scaling"). `report.md` is the latest night
against chapter 23.1's shares; `history/` holds one record per night and run, `YYYY-MM-DD-N.txt`,
in the format `geom_bench::timings` documents. Written by
`projects/engines/geometry/crates/geom-bench/reference/nightly.sh` on `main`; nothing here is edited
by hand.
README
  git -C "$work/timings" add README.md report.md "history/$night-$run.txt"
  git -C "$work/timings" commit -q -s -m "Reference timings: the night of $night, run $run (commit ${commit:0:12})"
  git -C "$work/timings" push -q origin "HEAD:refs/heads/$branch"
  say "pushed to $branch"
fi
