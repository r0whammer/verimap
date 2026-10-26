-- Human-authored specification of the kernel's value guards. Stated over
-- natural numbers and integers, independently of the Aeneas-generated model
-- in `VeriMap/Code`.
module

public import Aeneas

@[expose] public section

namespace VeriMap.Spec

/-- Largest valid confidence in basis points: 10 000 = 100 %. -/
def confidenceMax : Nat := 10000

/-- Smallest valid scene coordinate in millimetres (−100 km). -/
def coordMinMm : Int := -100000000

/-- Largest valid scene coordinate in millimetres (+100 km). -/
def coordMaxMm : Int := 100000000

/-- A confidence of `bp` basis points is valid iff it is at most 100 %. -/
def ValidConfidence (bp : Nat) : Prop := bp ≤ confidenceMax

/-- `v` millimetres is a valid scene coordinate. -/
def CoordInRange (v : Int) : Prop := coordMinMm ≤ v ∧ v ≤ coordMaxMm

/-- `[lo, hi]` is a valid closed axis interval: both ends are valid scene
coordinates and `lo ≤ hi` (a single point is allowed). -/
def ValidAxisBounds (lo hi : Int) : Prop :=
  CoordInRange lo ∧ CoordInRange hi ∧ lo ≤ hi

end VeriMap.Spec
