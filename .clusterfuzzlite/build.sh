#!/bin/bash -eu
# Builds the geometry kernel's fuzz targets for ClusterFuzzLite (kernel architecture, chapter 28.5;
# the layout is in projects/engines/geometry/crates/geom-fuzz/README.md). ClusterFuzzLite runs this
# script inside the image of .clusterfuzzlite/Dockerfile, after OSS-Fuzz's compile script has set
# its compiler environment, and fuzzes every target it finds under $OUT.
#
# A target is a bolero test of geom-fuzz whose name ends in `_fuzz` (tests/<target>.rs). cargo-bolero
# 0.13.5's build-clusterfuzz command builds geom-fuzz's tests with libFuzzer, the address sanitizer
# and coverage instrumentation, exactly as `cargo bolero test <name> --engine libfuzzer` does, and
# packs each test binary (named `<binary>.cargo`, which ClusterFuzzLite does not take for a target)
# with one start-up script `<test>_fuzzer` per bolero test, which passes libFuzzer's arguments to
# the test (bin/cargo-bolero/src/build_clusterfuzz.rs at commit
# 7642943b6c1f8a9b5e824dd1cd29b7c8e56068b8 of https://github.com/camshaft/bolero). So ClusterFuzzLite
# fuzzes the very binary cargo-bolero fuzzes, and both share the targets' corpus and crash files.
#
# OSS-Fuzz's build check (infra/base-images/base-runner/bad_build_check at commit
# ac54c4f21555fabcf24e19bad3213ac194ee9f85 of https://github.com/google/oss-fuzz) runs `file` and
# `objdump` on each target, so it cannot read a start-up script; the workflow turns it off
# (bad-build-check: false in .github/workflows/kernel-fuzz.yml) and this script makes the same
# checks on the binary each script starts, below, failing the build if any does not hold.

cd "$SRC/spareware"

# The reviewed lockfile, unchanged: the build resolves nothing new (chapter 28.13).
cargo metadata --locked --format-version 1 > /dev/null

# OSS-Fuzz's compile script sets C and C++ flags that add the sanitizer and coverage
# instrumentation to C code, and Rust flags that name the sanitizer and the fuzzing cfg.
# cargo-bolero adds the Rust flags itself, and bolero's libFuzzer engine compiles libFuzzer, which
# must not instrument itself: build it plainly with the system's GNU compiler, as on any Linux
# machine, and keep only OSS-Fuzz's debug information and frame pointers for the stack traces.
export CC=gcc CXX=g++ CFLAGS="" CXXFLAGS=""
export RUSTFLAGS="-Cdebuginfo=1 -Cforce-frame-pointers"

cargo bolero build-clusterfuzz --package geom-fuzz --sanitizer address

target_dir="$(cargo metadata --format-version 1 --no-deps | jq -r .target_directory)"
staging="$WORK/clusterfuzz"
rm -rf "$staging"
mkdir -p "$staging"
tar -xf "$target_dir/fuzz/clusterfuzz.tar" -C "$staging"

# Keep the `<target>_fuzz` tests' scripts, with the binary they start; drop every other bolero
# test's script (a proof's property test, a test that shows a failure is reported).
kept=0
for dir in "$staging"/*/; do
  name="$(basename "$dir")"
  scripts=("$dir"*_fuzz_fuzzer)
  [ -e "${scripts[0]}" ] || continue
  mkdir -p "$OUT/$name"
  cp "$dir"*.cargo "$OUT/$name/"
  for script in "${scripts[@]}"; do
    cp "$script" "$OUT/$name/"
    kept=$((kept + 1))
  done
done
if [ "$kept" -eq 0 ]; then
  echo "BAD BUILD: no geom-fuzz test named <target>_fuzz was found" >&2
  exit 1
fi

# OSS-Fuzz's build check for a libFuzzer target built with the address sanitizer, on each binary a
# kept script starts: an x86-64 ELF executable; at least 1000 calls into the address sanitizer;
# coverage instrumentation of at least 100 edges; and a clean start that runs 64 inputs.
for script in "$OUT"/*/*_fuzz_fuzzer; do
  binaries=("$(dirname "$script")"/*.cargo)
  binary="${binaries[0]}"
  target="$(basename "$script")"
  if ! readelf -h "$binary" | grep -q "Advanced Micro Devices X86-64"; then
    echo "BAD BUILD: $binary is not an x86-64 ELF executable" >&2
    exit 1
  fi
  asan_calls="$(objdump -dC "$binary" | grep -cE "callq?\s+[0-9a-f]+\s+<__asan" || true)"
  if [ "$asan_calls" -lt 1000 ]; then
    echo "BAD BUILD: $binary has $asan_calls calls into the address sanitizer, fewer than 1000" >&2
    exit 1
  fi
  log="$WORK/$target.check.log"
  if ! "$script" -runs=64 -seed=1337 > "$log" 2>&1; then
    echo "BAD BUILD: $target failed to start or crashed on its first inputs:" >&2
    cat "$log" >&2
    exit 1
  fi
  if ! grep -q "Done 64 runs" "$log"; then
    echo "BAD BUILD: $target did not run its first 64 inputs:" >&2
    cat "$log" >&2
    exit 1
  fi
  edges="$(grep -oP "INFO: Loaded [0-9]+ modules? +\([0-9]+ (inline 8-bit counters|guards)\)" "$log" \
    | grep -oP "\(\K[0-9]+" | head -n 1 || true)"
  if [ -z "$edges" ] || [ "$edges" -lt 100 ]; then
    echo "BAD BUILD: $target has coverage instrumentation of '${edges}' edges, fewer than 100:" >&2
    cat "$log" >&2
    exit 1
  fi
  echo "Build check: $target: x86-64 ELF, $asan_calls address-sanitizer calls, $edges edges, 64 runs."
done
