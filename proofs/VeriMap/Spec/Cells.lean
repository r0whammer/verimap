-- Cell and chunk indexing, stated over integers and independent of the
-- Aeneas model. Lean's `/` on `Int` is floor division (`ediv`). Rust's `/`
-- truncates toward zero (`tdiv`); the kernel's `floor_div_const` adjusts
-- the truncated quotient so the two agree for a positive divisor.
module

public import Aeneas
public import VeriMap.Spec.Bounds

@[expose] public section

namespace VeriMap.Spec

/-- One surface cell is one metre. -/
def cellSizeMm : Int := 1000

/-- Cells along one edge of a chunk. -/
def cellsPerChunk : Int := 32

/-- Smallest cell index `cell_index` returns. `coordMinMm` is divisible by
`cellSizeMm`, so this is exact. -/
def cellIndexMin : Int := coordMinMm / cellSizeMm

/-- Largest cell index `cell_index` returns. -/
def cellIndexMax : Int := coordMaxMm / cellSizeMm

/-- A cell index that `cell_index` can return. -/
def CellIndexInRange (c : Int) : Prop :=
  cellIndexMin ≤ c ∧ c ≤ cellIndexMax

/-- For `d > 0`, floor division is the truncated quotient, minus one when the
truncated remainder is negative. -/
theorem ediv_eq_tdiv_adjust (n d : Int) (hd : 0 < d) :
    n / d = n.tdiv d + if n.tmod d < 0 then -1 else 0 := by
  rw [Int.ediv_eq_tdiv]
  have hsign : Int.sign d = 1 := Int.sign_eq_one_of_pos hd
  rw [hsign]
  by_cases hr : n.tmod d < 0
  · have hn : ¬ (0 ≤ n) := by
      by_contra hnn
      have := Int.tmod_nonneg d hnn
      omega
    have hnd : ¬ d ∣ n := by
      intro h
      rw [Int.dvd_iff_tmod_eq_zero] at h
      omega
    have hcond : ¬ (0 ≤ n ∨ d ∣ n) := by
      intro h
      cases h <;> contradiction
    simp [hr, hcond]
    omega
  · have hcond : 0 ≤ n ∨ d ∣ n := by
      by_cases hn : 0 ≤ n
      · exact Or.inl hn
      · have hn' : n < 0 := by omega
        have htmod : n.tmod d = -((-n).tmod d) := by
          simp [Int.neg_tmod]
        have hnn : 0 ≤ -n := by omega
        have hpos : 0 ≤ (-n).tmod d := Int.tmod_nonneg d hnn
        have hz : n.tmod d = 0 := by omega
        refine Or.inr ?_
        rw [Int.dvd_iff_tmod_eq_zero]
        exact hz
    simp [hr, hcond]

/-- Splitting `n` by a positive `d` with a non-negative local offset
reconstructs `n`, and the quotient is floor division. -/
theorem split_chunk (n d : Int) (hd : 0 < d) :
    let q := n.tdiv d
    let r := n.tmod d
    let chunk := if r < 0 then q - 1 else q
    let localCell := if r < 0 then r + d else r
    chunk = n / d ∧ 0 ≤ localCell ∧ localCell < d ∧ chunk * d + localCell = n := by
  intro q r chunk localCell
  have hdiv := ediv_eq_tdiv_adjust n d hd
  have hrlt := Int.tmod_lt_of_pos n hd
  have _hrgt := Int.lt_tmod_of_pos n hd
  have heq := Int.tmod_add_mul_tdiv n d
  by_cases hr : r < 0
  · have hchunk : chunk = q - 1 := by simp [chunk, hr]
    have hlocal : localCell = r + d := by simp [localCell, hr]
    have hfloor : q + -1 = n / d := by
      simpa [q, r, hr] using hdiv.symm
    have hsub : q - 1 = q + -1 := by ring
    refine ⟨?_, ?_, ?_, ?_⟩
    · rw [hchunk, hsub, hfloor]
    · rw [hlocal]; omega
    · rw [hlocal]; omega
    · rw [hchunk, hlocal]
      have hsum : (q - 1) * d + (r + d) = r + d * q := by ring
      rw [hsum]
      simpa [q, r] using heq
  · have hchunk : chunk = q := by simp [chunk, hr]
    have hlocal : localCell = r := by simp [localCell, hr]
    have hfloor : q = n / d := by
      simpa [q, r, hr, Int.add_zero] using hdiv.symm
    refine ⟨?_, ?_, ?_, ?_⟩
    · rw [hchunk, hfloor]
    · rw [hlocal]; omega
    · rw [hlocal]; exact hrlt
    · rw [hchunk, hlocal]
      have hsum : q * d + r = r + d * q := by ring
      rw [hsum]
      simpa [q, r] using heq

open Aeneas.Std

/-- Truncating division by 1 m, then subtracting one, stays inside `i64` for a
scene coordinate. -/
theorem cell_tdiv_fits (n : Int) (h : -100000000 ≤ n ∧ n ≤ 100000000) :
    -9223372036854775808 ≤ n.tdiv 1000 - 1 ∧ n.tdiv 1000 - 1 ≤ 9223372036854775807 := by
  have hdivAbs : (n.tdiv 1000).natAbs ≤ n.natAbs := Int.natAbs_tdiv_le_natAbs n 1000
  have hnAbs : n.natAbs ≤ 100000000 := by
    rcases Int.natAbs_eq n with hn | hn
    · rw [hn]; omega
    · rw [hn, Int.natAbs_neg]; omega
  have : (n.tdiv 1000).natAbs ≤ 100000000 := le_trans hdivAbs hnAbs
  rcases Int.natAbs_eq (n.tdiv 1000) with hq | hq
  · rw [hq]; omega
  · rw [hq]; omega

/-- A cell index inside the proved range has a chunk split that fits in `i64`. -/
theorem chunk_split_fits (n : Int) (h : CellIndexInRange n) :
    I64.min ≤ n.tdiv 32 - 1 ∧ n.tdiv 32 - 1 ≤ I64.max ∧
      I64.min ≤ n.tmod 32 + 32 ∧ n.tmod 32 + 32 ≤ I64.max ∧
      (if n.tmod 32 < 0 then n.tdiv 32 - 1 else n.tdiv 32) = n / 32 ∧
      0 ≤ (if n.tmod 32 < 0 then n.tmod 32 + 32 else n.tmod 32) ∧
      (if n.tmod 32 < 0 then n.tmod 32 + 32 else n.tmod 32) < 32 ∧
      (if n.tmod 32 < 0 then n.tdiv 32 - 1 else n.tdiv 32) * 32 +
        (if n.tmod 32 < 0 then n.tmod 32 + 32 else n.tmod 32) = n := by
  have hd : (0 : Int) < 32 := by decide
  have hrange : -100000 ≤ n ∧ n ≤ 100000 := by
    simpa [CellIndexInRange, cellIndexMin, cellIndexMax, coordMinMm, coordMaxMm, cellSizeMm] using h
  have hsplit := split_chunk n 32 hd
  dsimp only at hsplit
  rcases hsplit with ⟨h1, h2, h3, h4⟩
  have hdivAbs : (n.tdiv 32).natAbs ≤ n.natAbs := Int.natAbs_tdiv_le_natAbs n 32
  have hq : -100000 ≤ n.tdiv 32 ∧ n.tdiv 32 ≤ 100000 := by
    have hnAbs : n.natAbs ≤ 100000 := by
      rcases Int.natAbs_eq n with hn | hn
      · rw [hn]; omega
      · rw [hn, Int.natAbs_neg]; omega
    have : (n.tdiv 32).natAbs ≤ 100000 := le_trans hdivAbs hnAbs
    rcases Int.natAbs_eq (n.tdiv 32) with hq | hq
    · rw [hq]; omega
    · rw [hq]; omega
  have hrgt : -32 < n.tmod 32 := Int.lt_tmod_of_pos n hd
  have hrlt : n.tmod 32 < 32 := Int.tmod_lt_of_pos n hd
  simp only [I64.min_eq, I64.max_eq]
  exact ⟨by omega, by omega, by omega, by omega, h1, h2, h3, h4⟩

end VeriMap.Spec
