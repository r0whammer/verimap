#!/usr/bin/env bash
# Exact-match tool pairs (docs/adr/0001-toolchain.md). Run inside `nix develop`.
set -euo pipefail
cd "$(dirname "$0")/.."
status=0

check() { # check <what> <expected> <actual>
  if [ "$2" != "$3" ]; then
    echo "toolchain-check: $1: expected $2, found $3"
    status=1
  fi
}

crate="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)"
check "wasm-bindgen crate (Cargo.lock) vs wasm-bindgen-cli" "$VERIMAP_WASM_BINDGEN_VERSION" "$crate"
check "wasm-bindgen-cli on PATH" "wasm-bindgen $VERIMAP_WASM_BINDGEN_VERSION" "$(wasm-bindgen --version)"

playwright="$(jq -r '.devDependencies["@playwright/test"]' web/package.json)"
check "@playwright/test (web/package.json) vs Nix playwright-driver" "$VERIMAP_PLAYWRIGHT_VERSION" "$playwright"

check "proofs/lean-toolchain vs Aeneas" "$(cat "$AENEAS_SOURCE/backends/lean/lean-toolchain")" "$(cat proofs/lean-toolchain)"
lake_rev="$(sed -n 's/^rev = "\(.*\)"$/\1/p' proofs/lakefile.toml)"
check "proofs/lakefile.toml Aeneas rev vs flake.lock" "$AENEAS_REV" "$lake_rev"
manifest_rev="$(jq -r '.packages[] | select(.name == "aeneas") | .rev' proofs/lake-manifest.json)"
check "proofs/lake-manifest.json Aeneas rev vs flake.lock" "$AENEAS_REV" "$manifest_rev"

[ "$status" -eq 0 ] && echo "toolchain-check: ok"
exit "$status"
