#!/usr/bin/env bash
# Proof audit; run after `lake build` (just audit does both). Fails when:
#   1. a human-authored module under Spec/ or Properties/ is not imported by
#      proofs/VeriMap.lean, so `lake build` would never check it;
#   2. human-authored Lean contains sorry, admit, native_decide, an axiom
#      declaration, or an extern/implemented_by escape (a source scan);
#   3. any theorem in those modules depends on an axiom other than propext,
#      Classical.choice, Quot.sound (VeriMap/Audit.lean, which inspects the
#      compiled theorems; the source scan alone is not sufficient).
set -euo pipefail
cd "$(dirname "$0")/../proofs"
status=0

while IFS= read -r f; do
  mod="${f%.lean}"
  mod="${mod//\//.}"
  if ! grep -qxF "public import $mod" VeriMap.lean; then
    echo "audit: $f is not imported by proofs/VeriMap.lean"
    status=1
  fi
done < <(find VeriMap/Spec VeriMap/Properties -name '*.lean' | sort)

human=(VeriMap.lean VeriMap/Spec VeriMap/Properties)
if grep -rnwE 'sorry|admit|native_decide' "${human[@]}" \
  || grep -rnE '^\s*(@\[[^]]*\]\s*)?((private|protected|noncomputable|unsafe)\s+)*axiom\s' "${human[@]}" \
  || grep -rnE '@\[[^]]*(extern|implemented_by)' "${human[@]}"; then
  echo "audit: forbidden construct in human-authored Lean (see above)"
  status=1
fi

if grep -rnE '^\s*axiom\s' VeriMap/Code; then
  echo "audit: note: generated code declares the axioms above; any theorem"
  echo "that depends on one fails the axiom check below"
fi

lake env lean VeriMap/Audit.lean || status=1
exit "$status"
