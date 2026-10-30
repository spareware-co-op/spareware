#!/usr/bin/env bash
# Builds the Clothoids oracle (piece G63; chapter 28.10's out-of-process oracles) and runs it:
#   clothoids.sh <clothoids checkout> <out dir> < cases > answers
# The checkout is Bertolazzi and Frego's Clothoids (BSD-2-Clause) at the commit the workflow pins,
# with its submodules. The library's sources, its three submodules' and the harness
# (clothoids_harness.cc, beside this script) are compiled straight to one program, in parallel, with
# no CMake: the harness reads the cases on standard input and writes the points on standard output.
# Nothing built here is a dependency of the kernel; the program runs as its own process.
set -euo pipefail
src=$1
out=$2
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$out/obj"
flags="-std=c++17 -O2 -include utility -I $src/src -I $src/submodules/UtilsLite/src \
  -I $src/submodules/quarticRootsFlocke/src -I $src/submodules/GenericContainer/include \
  -I $src/submodules/GenericContainer/src"
if [ ! -x "$out/harness" ]; then
  # GenericContainerSupport.cc reads files through parsers the harness never uses.
  for f in "$src"/src/*.cc "$src"/submodules/quarticRootsFlocke/src/*.cc \
    "$src"/submodules/UtilsLite/src/*.cc "$src"/submodules/GenericContainer/src/*.cc \
    "$here/clothoids_harness.cc"; do
    case "$f" in *GenericContainerSupport.cc) continue ;; esac
    echo "$f"
  done | xargs -P "$(nproc)" -I{} sh -c \
    "g++ $flags -c '{}' -o '$out/obj/'\$(echo '{}' | md5sum | cut -c1-12).o" >&2
  g++ -o "$out/harness" "$out"/obj/*.o -lpthread >&2
fi
"$out/harness"
