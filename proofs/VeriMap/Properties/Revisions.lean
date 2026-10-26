-- Properties of the authority sequence step and the single-slot revision
-- rules (docs/verification.md rows 2–5), about the Aeneas model of
-- `verimap_kernel::{next_sequence, slot_upsert, slot_delete,
-- base_revision_matches}`.
--
-- Every theorem is a total-correctness triple, so each also states that
-- the function returns normally (no panic or overflow) on every input.
module

public import VeriMap.Code.Funs
public import VeriMap.Spec.Slots

@[expose] public section

namespace VeriMap

open Aeneas Aeneas.Std Result
open VeriMap.Spec

/-! ## Authority sequence -/

@[simp, scalar_tac_simps, grind =, agrind =]
theorem SEQUENCE_MAX_val : SEQUENCE_MAX.val = U64.max := by
  simp [SEQUENCE_MAX, core.num.U64.MAX, U64.rMax, U64.max_eq]

/-- Below the maximum the next sequence is exactly one more; at the maximum
the step reports `SequenceExhausted` instead of wrapping. -/
theorem next_sequence_spec (last : U64) :
    next_sequence last ⦃ r => match r with
      | .Ok n => last.val < U64.max ∧ n.val = last.val + 1
      | .Err e => last.val = U64.max ∧ e = .SequenceExhausted ⦄ := by
  unfold next_sequence
  step*

/-! ## Upsert -/

/-- Only `Apply` changes the slot, and an applied upsert leaves the object
live at the write's revision. In particular a stale, duplicate,
conflicting, or tombstoned upsert preserves the slot (row 2). -/
theorem slot_upsert_changes_only_on_apply (s : Slot) (r : U64) (same : Bool) :
    slot_upsert s r same ⦃ t =>
      (t.decision = .Apply ∧ t.slot = .Live r) ∨
      (t.decision ≠ .Apply ∧ t.slot = s) ⦄ := by
  unfold slot_upsert
  cases s <;> simp only [] <;> (try split_ifs) <;> simp_all [WP.spec_ok]

/-- An upsert applies exactly when the object is vacant or live at a lower
revision: an accepted revision never decreases (row 5), and a deleted
object is never written again in its epoch (row 4). -/
theorem slot_upsert_apply_iff (s : Slot) (r : U64) (same : Bool) :
    slot_upsert s r same ⦃ t =>
      (t.decision = .Apply ↔ (s = .Vacant ∨ ∃ c, s = .Live c ∧ c.val < r.val)) ⦄ := by
  unfold slot_upsert
  cases s <;> simp only [] <;> (try split_ifs) <;> simp_all [WP.spec_ok]

/-- An older revision is `Stale`, whether the object is live or deleted. -/
theorem slot_upsert_older_is_stale (s : Slot) (r : U64) (same : Bool)
    (h : s ≠ .Vacant ∧ r.val < slotRevision s) :
    slot_upsert s r same ⦃ t => t.decision = .Stale ∧ t.slot = s ⦄ := by
  unfold slot_upsert
  cases s <;> simp_all [slotRevision]
  all_goals split_ifs <;> simp_all [WP.spec_ok]
  all_goals scalar_tac

/-- At the live revision, equal content is a `Duplicate` and different
content a `Conflict`; neither changes the slot (row 3). -/
theorem slot_upsert_same_revision (r : U64) (same : Bool) :
    slot_upsert (.Live r) r same ⦃ t =>
      t.decision = (if same then .Duplicate else .Conflict) ∧ t.slot = .Live r ⦄ := by
  unfold slot_upsert
  simp only []
  split_ifs <;> simp_all [WP.spec_ok]

/-- Applying the same upsert twice is the same as applying it once: the
second application is a `Duplicate` and leaves the slot as the first left
it (row 3). -/
theorem slot_upsert_idempotent (s : Slot) (r : U64) :
    slot_upsert s r true ⦃ t => t.decision = .Apply →
      slot_upsert t.slot r true ⦃ t' => t'.decision = .Duplicate ∧ t'.slot = t.slot ⦄ ⦄ := by
  apply WP.spec_mono (slot_upsert_changes_only_on_apply s r true)
  rintro t (⟨_, hslot⟩ | ⟨hne, _⟩) happly
  · rw [hslot]
    apply WP.spec_mono (slot_upsert_same_revision r true)
    intro t' ⟨hd, hs⟩
    exact ⟨by simpa using hd, hs⟩
  · exact absurd happly hne

/-- A tombstone is never overwritten by an upsert (row 4). -/
theorem slot_upsert_keeps_tombstone (d r : U64) (same : Bool) :
    slot_upsert (.Deleted d) r same ⦃ t => t.decision ≠ .Apply ∧ t.slot = .Deleted d ⦄ := by
  unfold slot_upsert
  simp only []
  split_ifs <;> simp_all [WP.spec_ok]

/-- The recorded revision never decreases (row 5). -/
theorem slot_upsert_revision_monotone (s : Slot) (r : U64) (same : Bool) :
    slot_upsert s r same ⦃ t => slotRevision s ≤ slotRevision t.slot ⦄ := by
  unfold slot_upsert
  cases s <;> simp only [] <;> (try split_ifs) <;> simp_all [WP.spec_ok, slotRevision]
  all_goals scalar_tac

/-! ## Delete -/

/-- Only `Apply` changes the slot, and an applied delete leaves a tombstone
at the deletion's revision (rows 2 and 4). -/
theorem slot_delete_changes_only_on_apply (s : Slot) (r : U64) :
    slot_delete s r ⦃ t =>
      (t.decision = .Apply ∧ t.slot = .Deleted r) ∨
      (t.decision ≠ .Apply ∧ t.slot = s) ⦄ := by
  unfold slot_delete
  cases s <;> simp only [] <;> (try split_ifs) <;> simp_all [WP.spec_ok]

/-- A delete applies exactly when the object is live at a lower revision. A
vacant object is `Missing`; a deleted one is never deleted again. -/
theorem slot_delete_apply_iff (s : Slot) (r : U64) :
    slot_delete s r ⦃ t =>
      (t.decision = .Apply ↔ ∃ c, s = .Live c ∧ c.val < r.val) ⦄ := by
  unfold slot_delete
  cases s <;> simp only [] <;> (try split_ifs) <;> simp_all [WP.spec_ok]

/-- Repeating a delete at its own revision is a `Duplicate` (row 3). -/
theorem slot_delete_repeat_is_duplicate (r : U64) :
    slot_delete (.Deleted r) r ⦃ t => t.decision = .Duplicate ∧ t.slot = .Deleted r ⦄ := by
  unfold slot_delete
  simp only []
  split_ifs <;> simp_all [WP.spec_ok]

/-- A tombstone stays a tombstone, with the same revision (row 4). -/
theorem slot_delete_keeps_tombstone (d r : U64) :
    slot_delete (.Deleted d) r ⦃ t => t.slot = .Deleted d ⦄ := by
  unfold slot_delete
  simp only []
  split_ifs <;> simp_all [WP.spec_ok]

/-- The recorded revision never decreases (row 5). -/
theorem slot_delete_revision_monotone (s : Slot) (r : U64) :
    slot_delete s r ⦃ t => slotRevision s ≤ slotRevision t.slot ⦄ := by
  unfold slot_delete
  cases s <;> simp only [] <;> (try split_ifs) <;> simp_all [WP.spec_ok, slotRevision]
  all_goals scalar_tac

/-! ## Edit preconditions -/

/-- An edit's base revision matches exactly when the object is live at that
revision. -/
theorem base_revision_matches_spec (s : Slot) (base : U64) :
    base_revision_matches s base ⦃ b => (b = true ↔ s = .Live base) ⦄ := by
  unfold base_revision_matches
  cases s <;> simp_all [WP.spec_ok]

end VeriMap
