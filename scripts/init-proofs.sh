#!/usr/bin/env bash
# Pin the Lean project to the Aeneas revision locked in flake.lock.
#
# Run from anywhere inside `nix develop`. Writes:
#   proofs/lean-toolchain  (copied from the pinned Aeneas Lean backend)
#   proofs/lakefile.toml   (requires the Aeneas backend at the same rev)
# Re-run only after a deliberate Aeneas upgrade, then `lake update` in
# proofs/, regenerate the model (just translate), and replay all proofs.
set -euo pipefail

: "${AENEAS_SOURCE:?run inside nix develop}"
: "${AENEAS_REV:?run inside nix develop}"

cd "$(dirname "$0")/.."
mkdir -p proofs

cp "$AENEAS_SOURCE/backends/lean/lean-toolchain" proofs/lean-toolchain
chmod u+w proofs/lean-toolchain

cat > proofs/lakefile.toml <<TOML
name = "VeriMap"
defaultTargets = ["VeriMap"]

[[require]]
name = "aeneas"
git = "https://github.com/AeneasVerif/aeneas"
rev = "$AENEAS_REV"
subDir = "backends/lean"

[[lean_lib]]
name = "VeriMap"
TOML

echo "proofs/lean-toolchain: $(cat proofs/lean-toolchain)"
echo "proofs/lakefile.toml: aeneas rev $AENEAS_REV"
