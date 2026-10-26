-- Property "validated values satisfy declared bounds" (docs/verification.md):
-- the generated kernel guards accept exactly the values `Spec.Bounds`
-- declares valid, keep accepted values unchanged, and report the documented
-- error otherwise.
--
-- `m ⦃ r => P r ⦄` is Aeneas's total-correctness triple: `m` terminates
-- normally (no panic, overflow, or divergence) with a value `r` satisfying
-- `P r`.
module

public import VeriMap.Code.Funs
public import VeriMap.Spec.Bounds

@[expose] public section

namespace VeriMap

open Aeneas Aeneas.Std Result
open VeriMap.Spec

/-! ## Generated constants -/

@[simp, scalar_tac_simps, grind =, agrind =]
theorem CONFIDENCE_MAX_val : CONFIDENCE_MAX.val = confidenceMax := by
  simp [CONFIDENCE_MAX, confidenceMax]

@[simp, scalar_tac_simps, grind =, agrind =]
theorem COORD_MIN_MM_val : COORD_MIN_MM.val = coordMinMm := by
  simp [COORD_MIN_MM, coordMinMm]

@[simp, scalar_tac_simps, grind =, agrind =]
theorem COORD_MAX_MM_val : COORD_MAX_MM.val = coordMaxMm := by
  simp [COORD_MAX_MM, coordMaxMm]

/-! ## Confidence -/

/-- `Confidence.new` returns normally on every input. It accepts exactly the
valid confidences and keeps the value unchanged; otherwise it reports
`ConfidenceOutOfRange`. -/
theorem Confidence.new_spec (bp : U16) :
    Confidence.new bp ⦃ r => match r with
      | .Ok c => ValidConfidence bp.val ∧ c.basis_points = bp
      | .Err e => ¬ ValidConfidence bp.val ∧ e = .ConfidenceOutOfRange ⦄ := by
  unfold Confidence.new
  split <;> simp only [WP.spec_ok, ValidConfidence, and_true] <;> scalar_tac

/-- Every accepted confidence is at most 10 000 basis points. -/
theorem Confidence.new_accepted_le_max (bp : U16) :
    Confidence.new bp ⦃ r => ∀ c, r = .Ok c → c.basis_points.val ≤ confidenceMax ⦄ := by
  apply WP.spec_mono (Confidence.new_spec bp)
  rintro r hr c rfl
  obtain ⟨hvalid, heq⟩ := hr
  rw [heq]
  exact hvalid

/-- The boundary value 10 000 is accepted unchanged. -/
theorem Confidence.new_max_accepted :
    Confidence.new 10000#u16 ⦃ r => ∃ c, r = .Ok c ∧ c.basis_points = 10000#u16 ⦄ := by
  apply WP.spec_mono (Confidence.new_spec 10000#u16)
  intro r hr
  match r, hr with
  | .Ok c, ⟨_, heq⟩ => exact ⟨c, rfl, heq⟩
  | .Err _, ⟨hinvalid, _⟩ => exact absurd (by simp [ValidConfidence, confidenceMax]) hinvalid

/-- Every value above 10 000 is rejected with `ConfidenceOutOfRange`. -/
theorem Confidence.new_rejects_above_max (bp : U16) (h : confidenceMax < bp.val) :
    Confidence.new bp ⦃ r => r = .Err .ConfidenceOutOfRange ⦄ := by
  apply WP.spec_mono (Confidence.new_spec bp)
  intro r hr
  match r, hr with
  | .Ok _, ⟨hvalid, _⟩ => exact absurd hvalid (by simp only [ValidConfidence]; omega)
  | .Err _, ⟨_, heq⟩ => rw [heq]

/-! ## Scene coordinates and axis bounds -/

/-- `coord_in_range` returns normally and decides `CoordInRange`. -/
@[step]
theorem coord_in_range_spec (v : I64) :
    coord_in_range v ⦃ b => (b = true ↔ CoordInRange v.val) ⦄ := by
  unfold coord_in_range
  split <;> simp only [WP.spec_ok, decide_eq_true_eq, CoordInRange] <;> scalar_tac

/-- `AxisBoundsMm.new` returns normally on every input. It accepts exactly the
valid intervals and keeps both ends unchanged; otherwise it reports
`CoordinateOutOfRange` when an end is out of range, else `BoundsInverted`. -/
theorem AxisBoundsMm.new_spec (lo hi : I64) :
    AxisBoundsMm.new lo hi ⦃ r => match r with
      | .Ok b => ValidAxisBounds lo.val hi.val ∧ b.min = lo ∧ b.max = hi
      | .Err .CoordinateOutOfRange => ¬ (CoordInRange lo.val ∧ CoordInRange hi.val)
      | .Err .BoundsInverted =>
          CoordInRange lo.val ∧ CoordInRange hi.val ∧ hi.val < lo.val
      | .Err .ConfidenceOutOfRange => False ⦄ := by
  unfold AxisBoundsMm.new
  step*
  all_goals simp_all [ValidAxisBounds]

/-- `AxisBoundsMm.contains` returns normally and decides membership in the
closed interval `[b.min, b.max]`. -/
theorem AxisBoundsMm.contains_spec (b : AxisBoundsMm) (v : I64) :
    AxisBoundsMm.contains b v ⦃ r => (r = true ↔ b.min.val ≤ v.val ∧ v.val ≤ b.max.val) ⦄ := by
  unfold AxisBoundsMm.contains
  split <;> simp only [WP.spec_ok, decide_eq_true_eq] <;> scalar_tac

end VeriMap
