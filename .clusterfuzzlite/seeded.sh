#!/bin/bash -eu
# ClusterFuzzLite's seeded run (gate 1, piece G43; kernel architecture, chapter 32.43): ClusterFuzzLite's
# own build_fuzzers and run_fuzzers, on this checkout as it stands, so that the crash geom-test's
# `trip clusterfuzzlite` plants (the libFuzzer leg's dropped carry in geom-num's wide addition) must
# be built and found by the gate that .github/workflows/kernel-fuzz.yml's job `clusterfuzzlite`
# runs. That job's actions cannot take a planted line: on GitHub, build_fuzzers clones the pull
# request afresh (`ExternalGithub.prepare_for_fuzzer_build`, infra/cifuzz/continuous_integration.py
# at commit fd3ab6015eb1527422d92d360003895ad55b4234 of https://github.com/google/oss-fuzz). So this
# script runs the images those actions run (actions/build_fuzzers/action.yml and
# actions/run_fuzzers/action.yml at commit 52ecc61cb587ee99c26825a112a21abf19c7448c of
# https://github.com/google/clusterfuzzlite, the commit the job pins), each with the environment
# its action.yml gives it, the job's inputs included, in ClusterFuzzLite's mode for other CI
# systems, which builds the source it is given (`ExternalGeneric`, the same file).
#
# What differs from the job, and why:
#  - CFL_PLATFORM=standalone, ClusterFuzzLite's platform outside a CI system, with WORKSPACE,
#    PROJECT_SRC_PATH and REPOSITORY, which its GitHub platform reads from GitHub's own variables.
#  - FILESTORE=no_filestore: no corpus or earlier build is downloaded or uploaded, so a crash is
#    never excused as one the base already had.
#  - KEEP_UNAFFECTED_FUZZ_TARGETS=true: every target is fuzzed, whatever the change.
#
# The containers reach the runner's Docker through its socket, as an action's container does, and
# see this checkout and the workspace at the same paths, so the containers ClusterFuzzLite starts
# beside its own (`--volumes-from`) see them too. `tests/attack_g43.rs` of geom-test holds each
# docker run to its action: the image and every input the job gives it.
#
# Usage, from the repository root on Linux with Docker:
#   cargo run --locked -q -p geom-test --bin trip -- clusterfuzzlite -- bash .clusterfuzzlite/seeded.sh
# Build the trip binary outside the checkout (CARGO_TARGET_DIR), or its target folder joins the
# image. ClusterFuzzLite's workspace goes in RUNNER_TEMP, or /tmp outside GitHub's runners.

src="$PWD"
work="$(mktemp -d "${RUNNER_TEMP:-/tmp}/clusterfuzzlite.XXXXXX")"
shared=(
  -v /var/run/docker.sock:/var/run/docker.sock
  -v "$src:$src"
  -v "$work:$work"
  -e CFL_PLATFORM=standalone
  -e FILESTORE=no_filestore
  -e WORKSPACE="$work"
  -e PROJECT_SRC_PATH="$src"
  -e REPOSITORY=spareware
)

# build_fuzzers/action.yml's environment: the job gives language, sanitizer and bad-build-check;
# the rest are the action's defaults.
docker run --rm "${shared[@]}" \
  -e LANGUAGE=rust \
  -e SANITIZER=address \
  -e BAD_BUILD_CHECK=false \
  -e DRY_RUN=false \
  -e UPLOAD_BUILD=false \
  -e LOW_DISK_SPACE=True \
  -e KEEP_UNAFFECTED_FUZZ_TARGETS=true \
  gcr.io/oss-fuzz-base/clusterfuzzlite-build-fuzzers:v1

# run_fuzzers/action.yml's environment: the job gives fuzz-seconds, mode and sanitizer; the rest
# are the action's defaults (its language is C++, as the job leaves it).
docker run --rm "${shared[@]}" \
  -e FUZZ_SECONDS=300 \
  -e MODE=code-change \
  -e SANITIZER=address \
  -e LANGUAGE=c++ \
  -e DRY_RUN=false \
  -e LOW_DISK_SPACE=True \
  -e REPORT_UNREPRODUCIBLE_CRASHES=False \
  -e OUTPUT_SARIF=false \
  -e MINIMIZE_CRASHES=False \
  -e PARALLEL_FUZZING=false \
  -e REPORT_TIMEOUTS=true \
  -e REPORT_OOMS=true \
  gcr.io/oss-fuzz-base/clusterfuzzlite-run-fuzzers:v1
