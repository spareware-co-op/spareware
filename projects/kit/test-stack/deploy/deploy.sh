#!/usr/bin/env bash
# Puts the test stack on a fresh Ubuntu server, or updates one. Run it from your own machine:
#
#   deploy/deploy.sh <server address> <settings file> [ssh key file]
#
# It sends this checkout's committed code and your settings file (stack.env.example shows what
# goes in it), then runs install-on-server.sh there. The first run takes about 20 minutes on two
# cores, nearly all of it compiling.
set -euo pipefail

if [ $# -lt 2 ]; then
  echo "usage: deploy/deploy.sh <server address> <settings file> [ssh key file]" >&2
  exit 1
fi
server="$1"
settings="$2"
key="${3:-$HOME/.ssh/id_ed25519_spare_oracle}"
user="${DEPLOY_USER:-ubuntu}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(git -C "$here" rev-parse --show-toplevel)"
ssh=(ssh -i "$key" -o StrictHostKeyChecking=accept-new "$user@$server")

[ -f "$settings" ] || { echo "no settings file at $settings" >&2; exit 1; }

"${ssh[@]}" 'sudo mkdir -p /opt/spare-stack/src /etc/spare-stack && sudo chmod 700 /etc/spare-stack'
git -C "$root" archive HEAD projects/kit/test-stack projects/spare-construct/demo/index.html design/ribbon \
  | "${ssh[@]}" 'sudo tar -x -C /opt/spare-stack/src'
"${ssh[@]}" 'sudo tee /etc/spare-stack/owner.env >/dev/null && sudo chmod 600 /etc/spare-stack/owner.env' \
  <"$settings"
"${ssh[@]}" 'sudo bash /opt/spare-stack/src/projects/kit/test-stack/deploy/install-on-server.sh'
