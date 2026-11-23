#!/usr/bin/env bash
# Fail if a public function in verimap-kernel is not named in the verification
# ledger. Accessors are named because the theorems about the constructors and
# predicates mention them. This does not rebuild proofs; `just audit` does.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
src="$root/crates/verimap-kernel/src/lib.rs"
ledger="$root/docs/verification.md"
status=0
names=0
total="$(grep -cE 'pub fn [A-Za-z0-9_]+' "$src")"

while IFS= read -r name; do
  names=$((names + 1))
  if ! grep -Eq "(^|[^A-Za-z0-9_])${name}([^A-Za-z0-9_]|$)" "$ledger"; then
    echo "proof-inventory: $name is not named in docs/verification.md" >&2
    status=1
  fi
done < <(grep -oE 'pub fn [A-Za-z0-9_]+' "$src" | awk '{print $3}' | sort -u)

if [[ "$status" -eq 0 ]]; then
  echo "proof-inventory: ${total} public kernel functions; all ${names} distinct names are in docs/verification.md"
fi
exit "$status"
