#!/usr/bin/env bash
# Builds the kernel's geometry for the browser into app/pkg-kernel: construct-regen's kernel builder
# over solidcore's engine (shapes-kernel), which the page loads when a document's edition is the
# kernel's. Its own workspace, so it builds from shapes-kernel's folder and lock. It needs the
# wasm32-unknown-unknown target and wasm-bindgen-cli at the version in shapes-kernel's Cargo.toml.
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
target="${CARGO_TARGET_DIR:-$here/shapes-kernel/target}"

cd "$here/shapes-kernel"
cargo build --locked --release --target wasm32-unknown-unknown
wasm-bindgen --target web --no-typescript --out-dir "$here/../app/pkg-kernel" \
  "$target/wasm32-unknown-unknown/release/construct_shapes_kernel.wasm"
