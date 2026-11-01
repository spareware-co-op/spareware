#!/usr/bin/env bash
# Builds the geometry for the browser into app/pkg, which is committed so the server builds without
# a WebAssembly toolchain. Run it whenever construct-regen's geometry changes. It needs the
# wasm32-unknown-unknown target and wasm-bindgen-cli at the version in construct-shapes' Cargo.toml.
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
root="$(cd "$here/../../.." && pwd)"
target="${CARGO_TARGET_DIR:-$root/target}"

cd "$root"
cargo build --release -p construct-shapes --target wasm32-unknown-unknown
wasm-bindgen --target web --no-typescript --out-dir "$here/../app/pkg" \
  "$target/wasm32-unknown-unknown/release/construct_shapes.wasm"
