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
| 2 | A stale, duplicate, conflicting, or tombstoned write leaves the object's slot unchanged; only `Apply` changes it. An upsert older than the recorded revision is `Stale` | `verimap_kernel::slot_upsert`, `verimap_kernel::slot_delete` | `VeriMap.slot_upsert`, `VeriMap.slot_delete` | none (all slots, `u64` revisions, `same_content`) | `Revisions.lean`: `slot_upsert_changes_only_on_apply`, `slot_upsert_older_is_stale`, `slot_delete_changes_only_on_apply` | the caller passes the slot it holds for that object | proof checked | see below |
| 3 | At the live revision, equal content is `Duplicate` and different content is `Conflict`; repeating an applied upsert is a no-op `Duplicate`; repeating a delete at its revision is `Duplicate` | `verimap_kernel::slot_upsert`, `verimap_kernel::slot_delete` | `VeriMap.slot_upsert`, `VeriMap.slot_delete` | none | `Revisions.lean`: `slot_upsert_same_revision`, `slot_upsert_idempotent`, `slot_delete_repeat_is_duplicate` | `same_content` is computed by the domain store (equal fix and source for tracks, equal annotation for markers); that comparison is tested, not proved | proof checked | see below |
| 4 | A tombstone is never overwritten or re-deleted: no upsert applies to a deleted slot, and a delete leaves it unchanged. Upsert applies exactly when the slot is vacant or live at a lower revision; delete exactly when live at a lower revision | `verimap_kernel::slot_upsert`, `verimap_kernel::slot_delete` | `VeriMap.slot_upsert`, `VeriMap.slot_delete` | one scene epoch (the slot type has no epoch) | `Revisions.lean`: `slot_upsert_keeps_tombstone`, `slot_delete_keeps_tombstone`, `slot_upsert_apply_iff`, `slot_delete_apply_iff` | the store keeps every tombstone for the session (tested) | proof checked | see below |
| 5 | The revision a slot records never decreases | `verimap_kernel::slot_upsert`, `verimap_kernel::slot_delete` | `VeriMap.slot_upsert`, `VeriMap.slot_delete` | none (`Vacant` counts as revision 0, `Spec/Slots.lean`) | `Revisions.lean`: `slot_upsert_revision_monotone`, `slot_delete_revision_monotone` | none | proof checked | see below |
| 6 | Invalid update has no effect (whole command or event) | — (the store's plan-then-apply is not in the kernel) | — | — | — | — | not modeled | — |
| 7 | The authority sequence step is `last + 1` below `u64::MAX` and reports `SequenceExhausted` at the maximum; it never wraps | `verimap_kernel::next_sequence` | `VeriMap.next_sequence` | none | `Revisions.lean`: `next_sequence_spec` | none | proof checked | see below |
| 8 | An edit's base revision matches exactly when the object is live at that revision | `verimap_kernel::base_revision_matches` | `VeriMap.base_revision_matches` | none | `Revisions.lean`: `base_revision_matches_spec` | none | proof checked | see below |

Rows 1a–1d are the "validated values satisfy declared bounds" property for
scalar guards. They do not by themselves prove that scene state only holds
valid values; that needs the domain store's use of the guards to enter the
model (or a justified composition argument), which it has not yet.

Rows 2–5, 7, and 8 are about **one transition of one object's slot**. The
domain store (`crates/verimap-domain/src/store.rs`) calls `slot_upsert` or
`slot_delete` for the object each committed event touches. It accepts the
event only on `Apply`, and stores the revision from the returned slot. The
authority calls `next_sequence` and `base_revision_matches`. That use is
covered by tests (`crates/verimap-domain/tests/scene.rs`, including a
property test that random journals replay to the same state), not by a
proof. No theorem here yet says that a whole scene, a batch, or a replay
has these properties.

Row 6 at the slot level is part of row 2: a refused decision returns the
input slot. The store-level claim needs more. A rejected event or command
must leave the entire scene unchanged, so the store validates the whole
event before mutating (`SceneStore::plan`, then commit). That is tested,
not modeled.

**Last checked:** rows 1a–1d were introduced and checked in commit
`896e1c5`. Rows 2–5, 7, and 8 were introduced in the M1 checkpoint commit
that follows `fc7a3a1`. On 2026-10-25, `just verify` checked rows 1a–8 on
that commit's tree. It regenerated the model from the current kernel,
built it with `lake build`, and audited 92 theorems, auxiliary lemmas
included.

Mutation check (2026-10-25): a planted resurrection bug in `slot_upsert`
applied a newer upsert over a tombstone. After regeneration, `lake build`
failed on `slot_upsert_apply_iff` and `slot_upsert_keeps_tombstone`. The
Rust tests `upsert_table` and `tombstones_are_permanent` failed too. After
the kernel was restored, the generated model was byte-identical and the
proofs built again.

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
