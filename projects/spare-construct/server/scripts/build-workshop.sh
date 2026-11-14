#!/usr/bin/env bash
# Builds Workshop's parts builder for the browser into demo/workshop/pkg, which is committed so the
# demo and the server need no WebAssembly toolchain. Run it whenever construct-workshop or the
# geometry kernel's facade changes. It needs the wasm32-unknown-unknown target and wasm-bindgen-cli
# at the version in construct-workshop's Cargo.toml.
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
root="$(cd "$here/../../.." && pwd)"
target="${CARGO_TARGET_DIR:-$root/target}"

cd "$root"
cargo build --release -p construct-workshop --target wasm32-unknown-unknown
wasm-bindgen --target web --no-typescript --out-dir "$here/../demo/workshop/pkg" \
  "$target/wasm32-unknown-unknown/release/construct_workshop.wasm"
