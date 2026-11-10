#!/usr/bin/env bash
# Check that docs/badges/*.json match measurements. Shields.io reads those
# files from the default branch; this script is what stops a hand-edited
# message from drifting away from CI.
#
#   scripts/check-badges.sh coverage   # needs target/coverage/gate.txt
#   scripts/check-badges.sh proofs lean
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

json_message() {
  python3 - "$1" << 'PY'
import json, sys
print(json.load(open(sys.argv[1]))["message"])
PY
}

check_coverage() {
  local gate="$root/target/coverage/gate.txt"
  if [[ ! -f "$gate" ]]; then
    echo "check-badges: $gate missing; run scripts/coverage.sh first" >&2
    exit 1
  fi
  local pct
  pct="$(sed -n 's/^coverage: production Rust .*(\([0-9.][0-9.]*\)%).*/\1/p' "$gate")"
  if [[ -z "$pct" ]]; then
    echo "check-badges: no production-Rust percentage in $gate" >&2
    exit 1
  fi
  local want="${pct}%"
  local got
  got="$(json_message docs/badges/coverage.json)"
  if [[ "$got" != "$want" ]]; then
    echo "check-badges: docs/badges/coverage.json says ${got}, measurement is ${want}" >&2
    exit 1
  fi
  echo "check-badges: coverage ${got}"
}

check_proofs() {
  scripts/proof-inventory.sh >/dev/null
  if grep -E '^\| .* \| (proof pending|model generated) \|' docs/verification.md; then
    echo "check-badges: a ledger row is not proof checked" >&2
    exit 1
  fi
  local n
  n="$(grep -cE 'pub fn [A-Za-z0-9_]+' crates/verimap-kernel/src/lib.rs)"
  local want="${n}/${n}"
  local got
  got="$(json_message docs/badges/proofs.json)"
  if [[ "$got" != "$want" ]]; then
    echo "check-badges: docs/badges/proofs.json says ${got}, kernel has ${want} public functions in the ledger" >&2
    exit 1
  fi
  echo "check-badges: kernel proofs ${got}"
}

check_lean() {
  local ver
  ver="$(sed -n 's/^leanprover\/lean4:v//p' proofs/lean-toolchain)"
  if [[ -z "$ver" ]]; then
    echo "check-badges: proofs/lean-toolchain has no leanprover/lean4 pin" >&2
    exit 1
  fi
  local got
  got="$(json_message docs/badges/lean.json)"
  if [[ "$got" != "$ver" ]]; then
    echo "check-badges: docs/badges/lean.json says ${got}, proofs/lean-toolchain pins ${ver}" >&2
    exit 1
  fi
  echo "check-badges: lean ${got}"
}

if [[ $# -eq 0 ]]; then
  echo "usage: scripts/check-badges.sh coverage|proofs|lean ..." >&2
  exit 2
fi
for arg in "$@"; do
  case "$arg" in
    coverage) check_coverage ;;
    proofs) check_proofs ;;
    lean) check_lean ;;
    *)
      echo "check-badges: unknown check ${arg}" >&2
      exit 2
      ;;
  esac
done
