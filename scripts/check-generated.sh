#!/usr/bin/env bash
# Fail if any committed generated output is stale, hand-modified, missing,
# extra, or untracked. Each generator writes into a fresh temporary
# directory, and the complete file set is compared with the working tree
# (`diff -r` reports changed files and files present on only one side).
# Run inside `nix develop`.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
status=0

compare() { # compare <committed dir> <fresh dir> <how to regenerate>
  if ! diff -r "$1" "$2"; then
    echo "check-generated: $1 differs from fresh output; run: $3"
    status=1
  fi
  local untracked
  untracked="$(git ls-files --others --exclude-standard -- "$1")"
  if [ -n "$untracked" ]; then
    echo "check-generated: untracked generated files (commit them):"
    echo "$untracked"
    status=1
  fi
}

scripts/translate.sh "$tmp/proofs"
compare proofs/VeriMap/Code "$tmp/proofs/VeriMap/Code" "just translate"

exit "$status"
