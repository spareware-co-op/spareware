#!/usr/bin/env bash
# Piece G35's Gappa pipeline, run inside the pinned Rocq container (kernel-certificates.yml):
# builds Gappa from its release tarball, installs the Gappa support library for Rocq from opam,
# then for each problem asks Gappa for its enclosures, writes Gappa's Rocq proof of them and
# checks that proof with Rocq. Usage: gappa.sh <problem.g>...
set -euo pipefail

# Gappa 1.8.3 (2026-09-09; CeCILL or GPL, run as a separate program, never linked).
GAPPA_VERSION=1.8.3
GAPPA_SHA256=9b3e14e95a5b6aa0ee909b1d1e5c1fac04042024f29b66377cfb9e1923b25f5c
# The Gappa support library for Rocq (LGPL-3.0-or-later, used only to check proofs).
COQ_GAPPA_VERSION=1.9.0

work=$(mktemp -d)
# The image's user installs system packages through sudo; a root user needs none.
SUDO=$(command -v sudo || true)
$SUDO apt-get update -qq
$SUDO env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq \
  curl g++ make flex bison libboost-dev libgmp-dev libmpfr-dev >/dev/null

curl -sSL -o "$work/gappa.tar.gz" \
  "https://gappa.gitlabpages.inria.fr/releases/gappa-$GAPPA_VERSION.tar.gz"
echo "$GAPPA_SHA256  $work/gappa.tar.gz" | sha256sum -c -
tar xzf "$work/gappa.tar.gz" -C "$work"
(cd "$work/gappa-$GAPPA_VERSION" && ./configure >/dev/null && ./remake -j"$(nproc)" >/dev/null)
gappa="$work/gappa-$GAPPA_VERSION/src/gappa"

eval "$(opam env)"
opam repo add rocq-released https://rocq-prover.org/opam/released >/dev/null 2>&1 || true
opam install -y "coq-gappa.$COQ_GAPPA_VERSION" >/dev/null
opam list --installed | grep -E 'rocq|coq|flocq|gappa'

status=0
for problem in "$@"; do
  name=$(basename "$problem" .g)
  echo "::group::Gappa on $name"
  # The enclosures Gappa proves, as it reports them.
  "$gappa" "$problem" || status=1
  # Its Rocq proof of the same, checked by Rocq.
  "$gappa" -Bcoq "$problem" >"$work/${name}_gappa.v" || status=1
  # Every axiom and assumption the final theorem rests on, as Rocq reports it.
  echo 'Print Assumptions l1.' >>"$work/${name}_gappa.v"
  (cd "$work" && coqc "${name}_gappa.v") && echo "Rocq checked Gappa's proof of $name" || status=1
  if [ -n "${OUT:-}" ]; then cp "$work/${name}_gappa.v" "$OUT/"; fi
  echo "::endgroup::"
done
exit "$status"
