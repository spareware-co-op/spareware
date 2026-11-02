#!/usr/bin/env bash
# Downloads the three outside programs the stack runs on, into $STACK_HOME/bin, without root:
#   TiKV (Rust, Apache-2.0), PD (Go, Apache-2.0), RustFS (Rust, Apache-2.0).
# Used for local testing and by deploy/deploy.sh. Works on x86-64 and ARM Linux.
# The TiKV and PD hashes are the ones in the signed TiUP mirror index for this version.
set -euo pipefail

TIKV_VERSION=v8.5.8
RUSTFS_VERSION=1.0.4
STACK_HOME="${STACK_HOME:-$HOME/spare-stack}"
mirror=https://tiup-mirrors.pingcap.com
rustfs_url=https://github.com/rustfs/rustfs/releases/download/$RUSTFS_VERSION

case "$(uname -m)" in
  x86_64)
    go_arch=amd64; rust_arch=x86_64
    tikv_sha=34eac1f0e794aa72039766174e691af4d4488286d7cabb7c0585766b03ffeed6
    pd_sha=ab07f7bc2f0285c30c047f933a54b95a6552d8a73e7302e6bf3ddeb567ff5750 ;;
  aarch64)
    go_arch=arm64; rust_arch=aarch64
    tikv_sha=24b73d3bc8af3258177e36242179753fa509c07b8b987b611cdf91d08c28e706
    pd_sha=29fc1334cfae384741ab27cb53df60ea8cbc1174273b395f1722319c6a9f40b9 ;;
  *) echo "unsupported machine: $(uname -m)" >&2; exit 1 ;;
esac

mkdir -p "$STACK_HOME/bin" "$STACK_HOME/download"
cd "$STACK_HOME/download"

if [ ! -x "$STACK_HOME/bin/tikv-server" ]; then
  curl -fsSLO "$mirror/tikv-$TIKV_VERSION-linux-$go_arch.tar.gz"
  echo "$tikv_sha  tikv-$TIKV_VERSION-linux-$go_arch.tar.gz" | sha256sum -c -
  tar -xzf "tikv-$TIKV_VERSION-linux-$go_arch.tar.gz" -C "$STACK_HOME/bin"
fi
if [ ! -x "$STACK_HOME/bin/pd-server" ]; then
  curl -fsSLO "$mirror/pd-$TIKV_VERSION-linux-$go_arch.tar.gz"
  echo "$pd_sha  pd-$TIKV_VERSION-linux-$go_arch.tar.gz" | sha256sum -c -
  tar -xzf "pd-$TIKV_VERSION-linux-$go_arch.tar.gz" -C "$STACK_HOME/bin"
fi
if [ ! -x "$STACK_HOME/bin/rustfs" ]; then
  zip="rustfs-linux-$rust_arch-gnu-v$RUSTFS_VERSION.zip"
  curl -fsSLO "$rustfs_url/$zip"
  curl -fsSLO "$rustfs_url/SHA256SUMS"
  grep " $zip\$" SHA256SUMS | sha256sum -c -
  unzip -oq "$zip" -d "$STACK_HOME/bin"
  chmod +x "$STACK_HOME/bin/rustfs"
fi

"$STACK_HOME/bin/tikv-server" --version | head -2
"$STACK_HOME/bin/pd-server" --version | head -1
"$STACK_HOME/bin/rustfs" --version | head -1
