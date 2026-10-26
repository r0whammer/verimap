-- Theorem-assumption audit, run by `just audit` (scripts/audit.sh) after
-- `lake build`. Not part of the library.
--
-- Enumerates every theorem declared in the human-authored modules
-- (`VeriMap.Spec.*`, `VeriMap.Properties.*`) from the compiled environment,
-- so a new theorem cannot be left out of the audit, and fails unless each
-- depends only on Lean's standard logical axioms. That rejects `sorry`
-- (`sorryAx`), `native_decide` (`Lean.ofReduceBool`), and any project or
-- generated declaration that is assumed rather than defined, however
-- indirectly a theorem reaches it.
import VeriMap
import Lean

open Lean Elab Command

/-- The only accepted axioms (docs/verification.md). -/
def allowedAxioms : List Name := [``propext, ``Classical.choice, ``Quot.sound]

/-- Modules whose theorems are audited. -/
def auditedModule (m : Name) : Bool :=
  (`VeriMap.Spec).isPrefixOf m || (`VeriMap.Properties).isPrefixOf m

elab "#audit_verimap_theorems" : command => do
  let env ← getEnv
  -- Identifiers avoid `name` and `axioms`: imported syntax makes them tokens.
  let mut checked : Nat := 0
  let mut failures : Array MessageData := #[]
  for i in [:env.header.moduleNames.size] do
    let some mod := env.header.moduleNames[i]? | continue
    unless auditedModule mod do continue
    let some data := env.header.moduleData[i]? | continue
    for decl in data.constNames do
      let some (.thmInfo _) := env.find? decl | continue
      let deps ← liftCoreM <| collectAxioms decl
      let unexpected := deps.filter (fun a => !allowedAxioms.contains a)
      checked := checked + 1
      -- Auxiliary theorems (equation lemmas, `_proof_N`, private helpers) are
      -- audited too, but only declared theorems are listed.
      unless decl.isInternalDetail do
        logInfo m!"{decl} depends on axioms: {deps.toList}"
      unless unexpected.isEmpty do
        failures := failures.push m!"{decl} depends on {unexpected.toList}"
  if checked == 0 then
    throwError "audit: no theorems found in VeriMap.Spec.* or VeriMap.Properties.*"
  unless failures.isEmpty do
    throwError m!"audit: unexpected axioms\n{MessageData.joinSep failures.toList "\n"}"
  logInfo m!"audit: {checked} theorems (including auxiliary ones) use only {allowedAxioms}"

#audit_verimap_theorems
