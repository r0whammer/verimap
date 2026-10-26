//! VeriMap's extracted kernel.
//!
//! Small, sequential, dependency-free guards that `verimap-domain` calls
//! before it accepts a value into scene state. Charon and Aeneas translate
//! this crate, and only this crate, to Lean (`proofs/VeriMap/Code`); the
//! theorems in `proofs/VeriMap/Properties` are about that generated model.
//! `docs/verification.md` lists what is proved and what is only tested.
//!
//! Rules for code in this crate:
//!
//! - Safe Rust, `no_std`, no dependencies, no allocation, no floating point,
//!   no wall clock.
//! - Stay inside the pinned Charon/Aeneas support envelope: plain structs and
//!   enums, `if`/`match`, integer comparisons. No traits beyond `Clone`/`Copy`
//!   derives, no closures, iterators, or collections yet.
//! - No `Debug` or `PartialEq` derives: their expansions would pull
//!   `core::fmt` and discriminant intrinsics into the translation as
//!   unverified external models. Callers inspect values through accessors.
//! - Malformed input returns a [`KernelError`]; nothing here panics.

#![no_std]
#![forbid(unsafe_code)]
// `(a..=b).contains(&v)` would bring `RangeInclusive` and `PartialOrd` trait
// calls into the translation; two plain comparisons translate directly.
#![allow(clippy::manual_range_contains)]

/// Why a kernel guard rejected its input.
#[derive(Clone, Copy)]
pub enum KernelError {
    /// A confidence above [`CONFIDENCE_MAX`] basis points.
    ConfidenceOutOfRange,
    /// A coordinate outside `COORD_MIN_MM..=COORD_MAX_MM`.
    CoordinateOutOfRange,
    /// An interval whose minimum is greater than its maximum.
    BoundsInverted,
}

/// Largest valid confidence: 10 000 basis points, i.e. 100 %.
pub const CONFIDENCE_MAX: u16 = 10_000;

/// A detection or track confidence in `0..=CONFIDENCE_MAX` basis points.
///
/// This is the estimator's reported score, not a calibrated probability; the
/// kernel guarantees only the range. Only [`Confidence::new`] constructs one.
#[derive(Clone, Copy)]
pub struct Confidence {
    basis_points: u16,
}

impl Confidence {
    /// Accepts `basis_points` iff it is at most [`CONFIDENCE_MAX`].
    pub fn new(basis_points: u16) -> Result<Confidence, KernelError> {
        if basis_points <= CONFIDENCE_MAX {
            Ok(Confidence { basis_points })
        } else {
            Err(KernelError::ConfidenceOutOfRange)
        }
    }

    /// The validated value in basis points.
    pub fn basis_points(self) -> u16 {
        self.basis_points
    }
}

/// Smallest valid scene coordinate: −100 km in millimetres.
///
/// Scene coordinates are ENU offsets from the session anchor, and a local
/// tangent frame is only meaningful near its origin. The ±100 km limit keeps
/// the difference of any two valid coordinates within ±2·10⁸, far inside
/// `i64`, so later cell and chunk arithmetic cannot overflow. Valid values are
/// also below 2⁵³ in magnitude, so they survive a JavaScript `Number` exactly.
pub const COORD_MIN_MM: i64 = -100_000_000;

/// Largest valid scene coordinate: +100 km in millimetres. See [`COORD_MIN_MM`].
pub const COORD_MAX_MM: i64 = 100_000_000;

/// Whether `v` is a valid scene coordinate, `COORD_MIN_MM..=COORD_MAX_MM`.
pub fn coord_in_range(v: i64) -> bool {
    COORD_MIN_MM <= v && v <= COORD_MAX_MM
}

/// A closed interval `[min, max]` of scene coordinates on one axis, in
/// millimetres. Only [`AxisBoundsMm::new`] constructs one, so both ends are
/// valid coordinates and `min <= max`.
#[derive(Clone, Copy)]
pub struct AxisBoundsMm {
    min: i64,
    max: i64,
}

impl AxisBoundsMm {
    /// Accepts `[min, max]` iff both ends satisfy [`coord_in_range`] and
    /// `min <= max`. A single-point interval (`min == max`) is accepted.
    /// When both checks fail, the range error is reported.
    pub fn new(min: i64, max: i64) -> Result<AxisBoundsMm, KernelError> {
        if !coord_in_range(min) || !coord_in_range(max) {
            Err(KernelError::CoordinateOutOfRange)
        } else if min > max {
            Err(KernelError::BoundsInverted)
        } else {
            Ok(AxisBoundsMm { min, max })
        }
    }

    /// Lower end, inclusive.
    pub fn min(self) -> i64 {
        self.min
    }

    /// Upper end, inclusive.
    pub fn max(self) -> i64 {
        self.max
    }

    /// Whether `v` lies in `[min, max]`.
    pub fn contains(self, v: i64) -> bool {
        self.min <= v && v <= self.max
    }
}
