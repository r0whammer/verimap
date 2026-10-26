#!/usr/bin/env bash
# Extract verimap-kernel (and only the kernel) with Charon, then translate the
# LLBC to Lean with Aeneas.
#
#   scripts/translate.sh [DEST]
#
# Writes DEST/VeriMap/Code/{Types,Funs}.lean; DEST defaults to proofs/.
# The output directory is replaced, so files for deleted Rust items vanish.
# Run inside `nix develop`.
set -euo pipefail

: "${AENEAS_SOURCE:?run inside nix develop}"

root="$(cd "$(dirname "$0")/.." && pwd)"
dest="$(mkdir -p "${1:-$root/proofs}" && cd "${1:-$root/proofs}" && pwd)"
llbc="$root/target/verimap_kernel.llbc"

cd "$root"
mkdir -p target
charon cargo --preset=aeneas --dest-file="$llbc" -- -p verimap-kernel --quiet

rm -rf "$dest/VeriMap/Code"
# Aeneas draws progress bars on a TTY; keep only its warnings and errors.
aeneas -backend lean "$llbc" -dest "$dest" -subdir /VeriMap/Code \
  -split-files -namespace VeriMap > "$root/target/aeneas.log" 2>&1 \
  || { cat "$root/target/aeneas.log"; exit 1; }
grep -iE 'warn|error' "$root/target/aeneas.log" || true

templates="$(find "$dest/VeriMap/Code" -name '*_Template.lean')"
if [ -n "$templates" ]; then
  echo "error: Aeneas generated external-model templates; each needs a reviewed"
  echo "model and an entry in docs/verification.md before it can be accepted:"
  echo "$templates"
  exit 1
fi
