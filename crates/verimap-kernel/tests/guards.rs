//! Boundary and property tests for the kernel guards. The Lean proofs are
//! about the Aeneas model of these functions; these tests exercise the
//! compiled Rust itself.

#![allow(clippy::panic)] // Test-only crate: helpers panic on unexpected results.

use proptest::prelude::*;
use verimap_kernel::{
    AxisBoundsMm, CONFIDENCE_MAX, COORD_MAX_MM, COORD_MIN_MM, Confidence, KernelError,
    coord_in_range,
};

fn accepted_confidence(raw: u16) -> Option<u16> {
    match Confidence::new(raw) {
        Ok(c) => Some(c.basis_points()),
        Err(KernelError::ConfidenceOutOfRange) => None,
        Err(_) => panic!("unexpected error kind for confidence {raw}"),
    }
}

fn bounds(min: i64, max: i64) -> Result<(i64, i64), &'static str> {
    match AxisBoundsMm::new(min, max) {
        Ok(b) => Ok((b.min(), b.max())),
        Err(KernelError::CoordinateOutOfRange) => Err("range"),
        Err(KernelError::BoundsInverted) => Err("inverted"),
        Err(KernelError::ConfidenceOutOfRange) => Err("confidence"),
    }
}

/// Builds bounds known to be valid; `unwrap` would need `KernelError: Debug`,
/// which the kernel deliberately does not derive.
fn valid_bounds(min: i64, max: i64) -> AxisBoundsMm {
    match AxisBoundsMm::new(min, max) {
        Ok(b) => b,
        Err(_) => panic!("[{min}, {max}] should be valid bounds"),
    }
}

#[test]
fn confidence_boundaries() {
    assert_eq!(accepted_confidence(0), Some(0));
    assert_eq!(accepted_confidence(CONFIDENCE_MAX), Some(10_000));
    assert_eq!(accepted_confidence(CONFIDENCE_MAX + 1), None);
    assert_eq!(accepted_confidence(u16::MAX), None);
}

#[test]
fn coordinate_boundaries() {
    assert!(coord_in_range(COORD_MIN_MM));
    assert!(coord_in_range(COORD_MAX_MM));
    assert!(coord_in_range(0));
    assert!(!coord_in_range(COORD_MIN_MM - 1));
    assert!(!coord_in_range(COORD_MAX_MM + 1));
    assert!(!coord_in_range(i64::MIN));
    assert!(!coord_in_range(i64::MAX));
}

// Valid coordinates are exact in a JavaScript Number (|v| <= 2^53 - 1), so
// the wire format may carry them as plain JSON numbers.
const JS_MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
const _: () = assert!(COORD_MAX_MM < JS_MAX_SAFE_INTEGER);
const _: () = assert!(-COORD_MIN_MM < JS_MAX_SAFE_INTEGER);

#[test]
fn axis_bounds_boundaries() {
    assert_eq!(bounds(-5, 5), Ok((-5, 5)));
    assert_eq!(bounds(7, 7), Ok((7, 7)));
    assert_eq!(
        bounds(COORD_MIN_MM, COORD_MAX_MM),
        Ok((COORD_MIN_MM, COORD_MAX_MM))
    );
    assert_eq!(bounds(1, 0), Err("inverted"));
    assert_eq!(bounds(COORD_MIN_MM - 1, 0), Err("range"));
    assert_eq!(bounds(0, COORD_MAX_MM + 1), Err("range"));
    // Both checks fail: the range error wins.
    assert_eq!(bounds(COORD_MAX_MM + 1, COORD_MIN_MM - 1), Err("range"));
}

#[test]
fn axis_bounds_contains_is_closed() {
    let b = valid_bounds(-100, 250);
    assert!(b.contains(-100));
    assert!(b.contains(250));
    assert!(b.contains(0));
    assert!(!b.contains(-101));
    assert!(!b.contains(251));
    assert!(!b.contains(i64::MIN));
    assert!(!b.contains(i64::MAX));
}

proptest! {
    #[test]
    fn confidence_accepts_exactly_the_valid_range(raw in any::<u16>()) {
        let expected = if raw <= CONFIDENCE_MAX { Some(raw) } else { None };
        prop_assert_eq!(accepted_confidence(raw), expected);
    }

    #[test]
    fn axis_bounds_accepts_exactly_valid_intervals(min in any::<i64>(), max in any::<i64>()) {
        let in_range = coord_in_range(min) && coord_in_range(max);
        let result = bounds(min, max);
        if !in_range {
            prop_assert_eq!(result, Err("range"));
        } else if min > max {
            prop_assert_eq!(result, Err("inverted"));
        } else {
            prop_assert_eq!(result, Ok((min, max)));
        }
    }

    #[test]
    fn axis_bounds_contains_matches_interval(
        a in COORD_MIN_MM..=COORD_MAX_MM,
        b in COORD_MIN_MM..=COORD_MAX_MM,
        v in any::<i64>(),
    ) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let bounds = valid_bounds(lo, hi);
        prop_assert_eq!(bounds.contains(v), lo <= v && v <= hi);
    }
}
