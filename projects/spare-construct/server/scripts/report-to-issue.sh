#!/usr/bin/env bash
# Turns a reproduced report into its generic public issue (piece A100, note A100-report-a-problem.md).
# For the co-op's operators, on the server that keeps the reports: it prints the issue the server
# makes from report <number> (its code, the commands its replay ran, the kinds of element, the
# engine and the version; no IDs, arguments, records, words or names), asks before it opens it with
# `gh issue create`, and then records the issue's address on the report, so its sender's state and
# the issue are linked. It never runs on its own and never on reports in a repository.
#
#   bash scripts/report-to-issue.sh <number> [<repository>]
set -euo pipefail

number="${1:?give the report's number}"
repository="${2:-spareware-co-op/spareware}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$here"
if [ -f config/local.env ]; then . config/local.env; fi
server="${SPARE_SERVER:-${CARGO_TARGET_DIR:-$here/target}/release/spare-server}"

text="$(mktemp)"
trap 'rm -f "$text"' EXIT
"$server" report issue "$number" >"$text"
title="$(head -n 1 "$text")"
body="$(mktemp)"
trap 'rm -f "$text" "$body"' EXIT
tail -n +3 "$text" >"$body"

echo "The issue report $number becomes, for $repository:"
echo
cat "$text"
echo
read -r -p "Open it? (yes to open) " answer
if [ "$answer" != yes ]; then
  echo "Not opened."
  exit 0
fi
address="$(gh issue create --repo "$repository" --title "$title" --body-file "$body")"
"$server" report linked "$number" "$address"
echo "Opened $address and recorded it on report $number."
