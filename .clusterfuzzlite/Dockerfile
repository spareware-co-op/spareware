# The image ClusterFuzzLite builds the geometry kernel's fuzz targets in (kernel architecture,
# chapter 28.5; projects/engines/geometry/crates/geom-fuzz/README.md). ClusterFuzzLite builds this
# file with the repository as its context and then runs .clusterfuzzlite/build.sh inside it.
#
# OSS-Fuzz's Rust builder, pinned to the digest its `latest` and `v1` tags named on 2026-10-24
# (pushed 2026-10-12), so the build cannot change under us. It sets RUSTUP_TOOLCHAIN to its own
# pinned nightly, which overrides the workspace's rust-toolchain.toml (rustup's override order), and
# provides clang and the OSS-Fuzz compile script that runs build.sh.
FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:3428d0a90b1e7efbe2ba947bd1d0c9a70594986b4b6553804547f1fb24cfc92f

# bolero's libFuzzer engine compiles its own copy of libFuzzer (C++, LLVM's, Apache-2.0 WITH
# LLVM-exception) with the system C++ compiler and links the GNU C++ library, as it does on any
# Linux machine; the OSS-Fuzz image ships clang and LLVM's C++ library but not g++.
RUN apt-get update && apt-get install -y --no-install-recommends g++ && rm -rf /var/lib/apt/lists/*

# cargo-bolero, whose build-clusterfuzz command packs the targets: the version the crate README
# names, built from its own published lockfile, with only the libFuzzer engine (the AFL engine's
# C sources are not needed).
RUN cargo install cargo-bolero --version 0.13.5 --locked --no-default-features --features libfuzzer

COPY . $SRC/spareware
WORKDIR $SRC/spareware
COPY .clusterfuzzlite/build.sh $SRC/
