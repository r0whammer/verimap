//! Cell and chunk indexing. Negative coordinates use floor division:
//! −1 mm is cell −1, and cell −1 is the last cell of chunk −1.

#![allow(clippy::panic)]

use proptest::prelude::*;
use verimap_kernel::{
    CELL_INDEX_MAX, CELL_INDEX_MIN, CELL_SIZE_MM, CELLS_PER_CHUNK, COORD_MAX_MM, COORD_MIN_MM,
    KernelError, cell_index, chunk_cell, coord_in_range,
};

/// `KernelError` has no `Debug` or `PartialEq` (those derives would enter the
/// extracted model). Compare through this tag instead.
fn cell(coord_mm: i64) -> Result<i64, &'static str> {
    match cell_index(coord_mm) {
        Ok(n) => Ok(n),
        Err(KernelError::CoordinateOutOfRange) => Err("range"),
        Err(KernelError::ConfidenceOutOfRange) => Err("confidence"),
        Err(KernelError::BoundsInverted) => Err("inverted"),
        Err(KernelError::SequenceExhausted) => Err("sequence"),
    }
}

fn reference_cell(coord_mm: i64) -> i64 {
    // Independent of the kernel: for a positive divisor, floor(n/d) is
    // −ceil(−n/d). Coordinates under test are far from `i64::MIN`.
    let n = i128::from(coord_mm);
    let d = i128::from(CELL_SIZE_MM);
    let q = if n >= 0 { n / d } else { -((d - 1 - n) / d) };
    i64::try_from(q).expect("cell index fits")
}

fn reference_chunk(cell: i64) -> (i64, i64) {
    let n = i128::from(cell);
    let d = i128::from(CELLS_PER_CHUNK);
    let q = if n >= 0 { n / d } else { -((d - 1 - n) / d) };
    let local = n - q * d;
    (
        i64::try_from(q).expect("chunk index fits"),
        i64::try_from(local).expect("local cell fits"),
    )
}

#[test]
fn cell_boundaries_use_floor_division() {
    assert_eq!(cell(0), Ok(0));
    assert_eq!(cell(999), Ok(0));
    assert_eq!(cell(1_000), Ok(1));
    assert_eq!(cell(-1), Ok(-1));
    assert_eq!(cell(-1_000), Ok(-1));
    assert_eq!(cell(-1_001), Ok(-2));
    assert_eq!(cell(COORD_MAX_MM), Ok(COORD_MAX_MM / CELL_SIZE_MM));
    assert_eq!(cell(COORD_MIN_MM), Ok(COORD_MIN_MM / CELL_SIZE_MM));
    assert_eq!(cell(COORD_MAX_MM + 1), Err("range"));
    assert_eq!(cell(COORD_MIN_MM - 1), Err("range"));
    assert_eq!(cell(i64::MIN), Err("range"));
    assert_eq!(cell(i64::MAX), Err("range"));
}

fn placed(cell_index: i64) -> Result<verimap_kernel::ChunkCell, &'static str> {
    match chunk_cell(cell_index) {
        Ok(c) => Ok(c),
        Err(KernelError::CoordinateOutOfRange) => Err("range"),
        Err(KernelError::ConfidenceOutOfRange) => Err("confidence"),
        Err(KernelError::BoundsInverted) => Err("inverted"),
        Err(KernelError::SequenceExhausted) => Err("sequence"),
    }
}

#[test]
fn chunk_edges_keep_a_non_negative_local_cell() {
    let check = |cell_index: i64, chunk: i64, local: i64| {
        let got = placed(cell_index).expect("in range");
        assert_eq!(got.chunk(), chunk, "{cell_index}");
        assert_eq!(got.local(), local, "{cell_index}");
        assert_eq!(chunk * CELLS_PER_CHUNK + local, cell_index);
    };
    check(0, 0, 0);
    check(31, 0, 31);
    check(32, 1, 0);
    check(-1, -1, 31);
    check(-32, -1, 0);
    check(-33, -2, 31);
    check(
        CELL_INDEX_MAX,
        reference_chunk(CELL_INDEX_MAX).0,
        reference_chunk(CELL_INDEX_MAX).1,
    );
    check(
        CELL_INDEX_MIN,
        reference_chunk(CELL_INDEX_MIN).0,
        reference_chunk(CELL_INDEX_MIN).1,
    );
    assert!(matches!(placed(CELL_INDEX_MIN - 1), Err("range")));
    assert!(matches!(placed(CELL_INDEX_MAX + 1), Err("range")));
    assert!(matches!(placed(i64::MIN), Err("range")));
    assert!(matches!(placed(i64::MAX), Err("range")));
}

proptest! {
    #[test]
    fn cell_index_matches_floor_division(v in any::<i64>()) {
        match cell(v) {
            Ok(got) => {
                prop_assert!(coord_in_range(v));
                prop_assert_eq!(got, reference_cell(v));
            }
            Err("range") => prop_assert!(!coord_in_range(v)),
            Err(other) => panic!("cell_index reported the wrong error for {v}: {other}"),
        }
    }

    #[test]
    fn chunk_cell_reconstructs_in_range_cells(cell_index in CELL_INDEX_MIN..=CELL_INDEX_MAX) {
        let got = placed(cell_index).expect("in range");
        let (chunk, local) = reference_chunk(cell_index);
        prop_assert_eq!(got.chunk(), chunk);
        prop_assert_eq!(got.local(), local);
        prop_assert!(got.local() >= 0 && got.local() < CELLS_PER_CHUNK);
        prop_assert_eq!(got.chunk() * CELLS_PER_CHUNK + got.local(), cell_index);
    }
}
