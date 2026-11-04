-- Cell and chunk indexing (docs/verification.md rows 9a–9c), about the
-- Aeneas model of `verimap_kernel::{cell_index, chunk_cell}` and the
-- `ChunkCell` accessors.
--
-- Every theorem is a total-correctness triple: the function returns
-- normally (no panic or overflow) and the value matches floor division.
module

public import VeriMap.Code.Funs
public import VeriMap.Properties.Guards
public import VeriMap.Spec.Cells

@[expose] public section

namespace VeriMap

open Aeneas Aeneas.Std Result
open VeriMap.Spec

/-! ## Generated constants -/

@[simp, scalar_tac_simps, grind =, agrind =]
theorem CELL_SIZE_MM_val : CELL_SIZE_MM.val = cellSizeMm := by
  simp [CELL_SIZE_MM, cellSizeMm]

@[simp, scalar_tac_simps, grind =, agrind =]
theorem CELLS_PER_CHUNK_val : CELLS_PER_CHUNK.val = cellsPerChunk := by
  simp [CELLS_PER_CHUNK, cellsPerChunk]

/-- `CELL_INDEX_MIN` is the exact floor quotient of the coordinate minimum. -/
@[step]
theorem CELL_INDEX_MIN_spec : CELL_INDEX_MIN ⦃ i => i.val = cellIndexMin ⦄ := by
  unfold CELL_INDEX_MIN
  have hnz : CELL_SIZE_MM.val ≠ (0 : Int) := by simp [CELL_SIZE_MM_val, cellSizeMm]
  have hov : ¬ (COORD_MIN_MM.val = I64.min ∧ CELL_SIZE_MM.val = -1) := by
    simp [CELL_SIZE_MM_val, cellSizeMm, I64.min_eq]
  have hdiv :
      (COORD_MIN_MM / CELL_SIZE_MM) ⦃ i => i.val = Int.tdiv COORD_MIN_MM.val CELL_SIZE_MM.val ⦄ :=
    I64.div_spec (x := COORD_MIN_MM) (y := CELL_SIZE_MM) hnz hov
  apply WP.spec_mono hdiv
  intro i hi
  simp only [COORD_MIN_MM_val, CELL_SIZE_MM_val, coordMinMm, cellSizeMm, cellIndexMin] at hi ⊢
  rw [hi]
  decide

/-- `CELL_INDEX_MAX` is the exact floor quotient of the coordinate maximum. -/
@[step]
theorem CELL_INDEX_MAX_spec : CELL_INDEX_MAX ⦃ i => i.val = cellIndexMax ⦄ := by
  unfold CELL_INDEX_MAX
  have hnz : CELL_SIZE_MM.val ≠ (0 : Int) := by simp [CELL_SIZE_MM_val, cellSizeMm]
  have hov : ¬ (COORD_MAX_MM.val = I64.min ∧ CELL_SIZE_MM.val = -1) := by
    simp [CELL_SIZE_MM_val, cellSizeMm, I64.min_eq]
  have hdiv :
      (COORD_MAX_MM / CELL_SIZE_MM) ⦃ i => i.val = Int.tdiv COORD_MAX_MM.val CELL_SIZE_MM.val ⦄ :=
    I64.div_spec (x := COORD_MAX_MM) (y := CELL_SIZE_MM) hnz hov
  apply WP.spec_mono hdiv
  intro i hi
  simp only [COORD_MAX_MM_val, CELL_SIZE_MM_val, coordMaxMm, cellSizeMm, cellIndexMax] at hi ⊢
  rw [hi]
  decide

/-! ## Cell index -/

/-- `cell_index` returns normally on every input. In range it is floor
division by 1 m; otherwise it reports `CoordinateOutOfRange` and no other
error. -/
theorem cell_index_spec (coord : I64) :
    cell_index coord ⦃ r => match r with
      | .Ok i => CoordInRange coord.val ∧ i.val = coord.val / cellSizeMm
      | .Err .CoordinateOutOfRange => ¬ CoordInRange coord.val
      | .Err .ConfidenceOutOfRange => False
      | .Err .BoundsInverted => False
      | .Err .SequenceExhausted => False ⦄ := by
  unfold cell_index floor_div_const
  step with coord_in_range_spec as ⟨ b ⟩
  split
  · rename_i hiff hin
    have hnz : CELL_SIZE_MM.val ≠ (0 : Int) := by simp [CELL_SIZE_MM_val, cellSizeMm]
    have hov : ¬ (coord.val = I64.min ∧ CELL_SIZE_MM.val = -1) := by
      simp [CELL_SIZE_MM_val, cellSizeMm]
    step with I64.div_spec (x := coord) (y := CELL_SIZE_MM) hnz hov as ⟨ q ⟩
    step with I64.rem_spec (x := coord) (y := CELL_SIZE_MM) hnz as ⟨ r ⟩
    have hfit := cell_tdiv_fits coord.val (by
      have : CoordInRange coord.val := hiff.mp hin
      simpa [CoordInRange, coordMinMm, coordMaxMm] using this)
    have hadj := ediv_eq_tdiv_adjust coord.val 1000 (by decide)
    simp [WP.spec_ok, CELL_SIZE_MM_val, cellSizeMm, CoordInRange, I64.min_eq, I64.max_eq] at *
    -- The remainder is zero, or it is negative and the quotient steps down by one.
    split
    · simp_all [hadj, hfit]
    · split
      · step with I64.sub_spec (x := q) (y := 1#i64)
          (by simp [I64.min_eq]; omega) (by simp [I64.max_eq]; omega) as ⟨ s ⟩
        have hlt : coord.val.tmod 1000 < 0 := by
          have heq : (-coord.val).tmod 1000 = -(coord.val.tmod 1000) := by
            rw [Int.neg_tmod]
          have hnn : 0 ≤ (-coord.val).tmod 1000 :=
            @Int.tmod_nonneg (-coord.val) 1000 (by omega)
          have hne : coord.val.tmod 1000 ≠ 0 := by simp_all
          omega
        rw [ite_eq_left hlt] at hadj
        have hrng := hiff.mp hin
        have hs : s.val = coord.val.tdiv 1000 - 1 := by omega
        refine ⟨hrng.1, hrng.2, ?_⟩
        rw [hs, hadj]
        ring
      · simp
        have hnn : 0 ≤ coord.val.tmod 1000 :=
          @Int.tmod_nonneg coord.val 1000 (by omega)
        simp_all [hnn, hadj]
  · simp only [WP.spec_ok]
    simp_all [CoordInRange]

/-- An accepted cell index lies in `[cellIndexMin, cellIndexMax]`. -/
theorem cell_index_in_range (coord : I64) :
    cell_index coord ⦃ r => ∀ i, r = .Ok i → CellIndexInRange i.val ⦄ := by
  apply WP.spec_mono (cell_index_spec coord)
  intro r hr i hi
  match r, hr with
  | .Ok got, ⟨hin, heq⟩ =>
      injection hi with h
      subst h
      have hlo : coordMinMm / cellSizeMm ≤ coord.val / cellSizeMm :=
        Int.ediv_le_ediv (by simp [cellSizeMm]) hin.1
      have hhi : coord.val / cellSizeMm ≤ coordMaxMm / cellSizeMm :=
        Int.ediv_le_ediv (by simp [cellSizeMm]) hin.2
      simpa [CellIndexInRange, cellIndexMin, cellIndexMax, heq] using And.intro hlo hhi
  | .Err _, _ => cases hi

/-! ## Chunk cell -/

/-- `chunk` returns the stored chunk index. -/
theorem ChunkCell.chunk_spec (c : ChunkCell) :
    ChunkCell.impl.chunk c ⦃ r => r = c.chunk ⦄ := by
  unfold ChunkCell.impl.chunk
  simp [WP.spec_ok]

/-- `local` returns the stored offset, in `0..CELLS_PER_CHUNK` when `chunk_cell`
built the value. -/
theorem ChunkCell.local_spec (c : ChunkCell) :
    ChunkCell.impl.local c ⦃ r => r = c.«local» ⦄ := by
  unfold ChunkCell.impl.local
  simp [WP.spec_ok]

/-- `chunk_cell` returns normally on every input. Inside the cell-index range
the chunk index is floor division by 32 and the local offset is in
`0..32`, and `chunk * 32 + local` is the cell. Outside the range it reports
`CoordinateOutOfRange`. -/
theorem chunk_cell_spec (cell : I64) :
    chunk_cell cell ⦃ r => match r with
      | .Ok c =>
          CellIndexInRange cell.val ∧
          c.chunk.val = cell.val / cellsPerChunk ∧
          0 ≤ c.«local».val ∧ c.«local».val < cellsPerChunk ∧
          c.chunk.val * cellsPerChunk + c.«local».val = cell.val
      | .Err .CoordinateOutOfRange => ¬ CellIndexInRange cell.val
      | .Err .ConfidenceOutOfRange => False
      | .Err .BoundsInverted => False
      | .Err .SequenceExhausted => False ⦄ := by
  unfold chunk_cell
  step*
  all_goals simp_all [CellIndexInRange, cellsPerChunk, CELLS_PER_CHUNK_val, I64.min_eq, I64.max_eq]
  all_goals
    have h := chunk_split_fits cell.val (by simp_all [CellIndexInRange])
    simp only [I64.min_eq, I64.max_eq] at h
    omega

end VeriMap
