# Verification ledger

What is formally checked, about which production code, and under what
assumptions. Status values: **not modeled**, **model generated**, **proof
pending**, **proof checked**. A row is *proof checked* only when the theorem
is about the Aeneas model regenerated from the current Rust
(`just check-generated`), is imported by `proofs/VeriMap.lean`, builds with
`lake build`, and passes `just audit`.

## Trust boundary

A checked theorem states a property of the Lean model that Charon and Aeneas
generate from `crates/verimap-kernel`. It relies on, and does not check:

- Charon's and Aeneas's translation of Rust semantics (pinned revisions in
  `flake.lock`; Aeneas `74aabd52…`, Charon `260ba2c8…`);
- Aeneas's Lean library models of Rust scalars, `Result`, and control flow;
- rustc (`nightly-2026-09-17`) code generation for the native and
  `wasm32-unknown-unknown` targets, and the browser's WebAssembly engine;
- the Lean 4 kernel (`v4.35.0`) and the standard axioms `propext`,
  `Classical.choice`, `Quot.sound`, which are the only ones accepted;
- how the domain, WASM bindings, and UI use the kernel. Integration is tested,
  not proved.

No theorem here says anything about confidence calibration, detection
accuracy, geodesy, rendering, persistence, or the application as a whole.

## Properties

| # | Property | Production symbol | Generated Lean symbol | Preconditions | Theorems (`proofs/VeriMap/Properties/`) | Extra assumptions | Status | Last checked |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1a | Accepted confidence is within `0..=10000` bp and unchanged; everything above is rejected with `ConfidenceOutOfRange`; never panics | `verimap_kernel::Confidence::new` | `VeriMap.Confidence.new` | none (all `u16`) | `Guards.lean`: `Confidence.new_spec`, `Confidence.new_accepted_le_max`, `Confidence.new_max_accepted`, `Confidence.new_rejects_above_max` | none | proof checked | see below |
| 1b | Coordinate guard decides `-10⁸ ≤ v ≤ 10⁸` mm; never panics | `verimap_kernel::coord_in_range` | `VeriMap.coord_in_range` | none (all `i64`) | `Guards.lean`: `coord_in_range_spec` | none | proof checked | see below |
| 1c | Axis bounds accept exactly valid intervals (ends in range, `min ≤ max`) and keep both ends; out-of-range ends report `CoordinateOutOfRange`, otherwise inversion reports `BoundsInverted`; never `ConfidenceOutOfRange`; never panics | `verimap_kernel::AxisBoundsMm::new` | `VeriMap.AxisBoundsMm.new` | none (all `i64` pairs) | `Guards.lean`: `AxisBoundsMm.new_spec` | none | proof checked | see below |
| 1d | Interval membership decides `min ≤ v ≤ max`; never panics | `verimap_kernel::AxisBoundsMm::contains` | `VeriMap.AxisBoundsMm.contains` | none | `Guards.lean`: `AxisBoundsMm.contains_spec` | none | proof checked | see below |
| 2 | Stale update preserves state | — (single-slot revision decision, M1) | — | — | — | — | not modeled | — |
| 3 | Duplicate application is idempotent; same revision with different content is a conflict | — | — | — | — | — | not modeled | — |
| 4 | Deletion resists stale resurrection | — | — | — | — | — | not modeled | — |
| 5 | Accepted revision never decreases | — | — | — | — | — | not modeled | — |
| 6 | Invalid update has no effect | — | — | — | — | — | not modeled | — |

Rows 1a–1d are the "validated values satisfy declared bounds" property for
scalar guards. They do not by themselves prove that scene state only holds
valid values; that needs the domain store's use of the guards to enter the
model (or a justified composition argument), which it has not yet.

**Last checked:** rows 1a–1d were introduced and checked in commit
`896e1c5`. On 2026-10-25, `just verify` (regeneration, `lake build`, and the
audit) checked them again on the tree of the commit that follows `896e1c5`.
The kernel source did not change between the two.

## Tested, not proved

Deterministic replay, checkpoint/full-replay equivalence, atomic
persistence, coordinate transforms, protocol parsing, and transport recovery
are covered (as they land) by Rust/property/browser tests, not by Lean.

## Audit

`just audit` (scripts/audit.sh, proofs/VeriMap/Audit.lean):

1. every `.lean` file under `VeriMap/Spec` and `VeriMap/Properties` is
   imported by `proofs/VeriMap.lean`, so `lake build` checks it;
2. human-authored Lean contains no `sorry`, `admit`, `native_decide`, axiom
   declarations, or `extern`/`implemented_by` attributes (source scan);
3. every theorem compiled from those modules, including auxiliary lemmas,
   depends only on `propext`, `Classical.choice`, `Quot.sound`
   (`Lean.collectAxioms` over the compiled environment; not a text scan).

On 2026-10-25 the audit was shown to reject a planted `sorry`
(`sorryAx`) and a planted `native_decide` (its auxiliary axiom) through
check 3 alone. Note that `lake build` accepts a `sorry` with only a warning,
so the audit is a required step.
