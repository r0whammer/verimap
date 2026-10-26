//! VeriMap's extracted kernel.
//!
//! Small, sequential, dependency-free guards and transitions that
//! `verimap-domain` calls before it accepts a value into scene state: value
//! bounds, the authority sequence step, and the per-object revision and
//! tombstone rules ([`slot_upsert`], [`slot_delete`]). Charon and Aeneas translate
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
    /// The authority sequence is at [`SEQUENCE_MAX`]; no further event can
    /// be committed in this epoch.
    SequenceExhausted,
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

/// Largest authority sequence number. Sequence 0 means "no event yet", so
/// the first committed event is 1.
pub const SEQUENCE_MAX: u64 = u64::MAX;

/// The authority sequence after `last`, or [`KernelError::SequenceExhausted`]
/// at [`SEQUENCE_MAX`]. Sequences never wrap around.
pub fn next_sequence(last: u64) -> Result<u64, KernelError> {
    if last < SEQUENCE_MAX {
        Ok(last + 1)
    } else {
        Err(KernelError::SequenceExhausted)
    }
}

/// What the reducer knows about one object ID within a scene epoch.
///
/// A revision is the authority sequence of the committed event that last
/// changed the object. A deleted object keeps a tombstone for the rest of
/// the epoch, so no later write can bring it back.
#[derive(Clone, Copy)]
pub enum Slot {
    /// No event in this epoch has written the object.
    Vacant,
    /// The object exists; last changed at `revision`.
    Live {
        /// Sequence of the last change.
        revision: u64,
    },
    /// The object was deleted at `revision`.
    Deleted {
        /// Sequence of the deletion.
        revision: u64,
    },
}

/// How a write relates to an object's current [`Slot`]. Only
/// [`Decision::Apply`] changes the slot.
#[derive(Clone, Copy)]
pub enum Decision {
    /// Newer than the current revision: apply it.
    Apply,
    /// The same write at the same revision was already applied: a no-op.
    Duplicate,
    /// Older than the current revision: ignore it.
    Stale,
    /// The current revision, but different content: reject it.
    Conflict,
    /// The object was deleted earlier in this epoch: reject it.
    Tombstoned,
    /// A delete of an object this epoch has never seen: reject it.
    Missing,
}

/// A decision and the slot that results from it.
#[derive(Clone, Copy)]
pub struct Transition {
    /// How the write relates to the previous slot.
    pub decision: Decision,
    /// The slot after the write; equal to the previous slot unless the
    /// decision is [`Decision::Apply`].
    pub slot: Slot,
}

/// Decides a create-or-replace write at `revision`. `same_content` says
/// whether the write's payload equals the stored one; it matters only when
/// the revisions are equal.
pub fn slot_upsert(slot: Slot, revision: u64, same_content: bool) -> Transition {
    match slot {
        Slot::Vacant => Transition {
            decision: Decision::Apply,
            slot: Slot::Live { revision },
        },
        Slot::Live { revision: current } => {
            if revision > current {
                Transition {
                    decision: Decision::Apply,
                    slot: Slot::Live { revision },
                }
            } else if revision < current {
                Transition {
                    decision: Decision::Stale,
                    slot,
                }
            } else if same_content {
                Transition {
                    decision: Decision::Duplicate,
                    slot,
                }
            } else {
                Transition {
                    decision: Decision::Conflict,
                    slot,
                }
            }
        }
        Slot::Deleted { revision: deleted } => {
            if revision < deleted {
                Transition {
                    decision: Decision::Stale,
                    slot,
                }
            } else if revision == deleted {
                Transition {
                    decision: Decision::Conflict,
                    slot,
                }
            } else {
                Transition {
                    decision: Decision::Tombstoned,
                    slot,
                }
            }
        }
    }
}

/// Decides a deletion at `revision`.
pub fn slot_delete(slot: Slot, revision: u64) -> Transition {
    match slot {
        Slot::Vacant => Transition {
            decision: Decision::Missing,
            slot,
        },
        Slot::Live { revision: current } => {
            if revision > current {
                Transition {
                    decision: Decision::Apply,
                    slot: Slot::Deleted { revision },
                }
            } else if revision < current {
                Transition {
                    decision: Decision::Stale,
                    slot,
                }
            } else {
                Transition {
                    decision: Decision::Conflict,
                    slot,
                }
            }
        }
        Slot::Deleted { revision: deleted } => {
            if revision < deleted {
                Transition {
                    decision: Decision::Stale,
                    slot,
                }
            } else if revision == deleted {
                Transition {
                    decision: Decision::Duplicate,
                    slot,
                }
            } else {
                Transition {
                    decision: Decision::Tombstoned,
                    slot,
                }
            }
        }
    }
}

/// Whether an edit made against `base_revision` may proceed: the object
/// exists and has not changed since. Operator edits carry this precondition
/// so they never silently overwrite a newer change.
pub fn base_revision_matches(slot: Slot, base_revision: u64) -> bool {
    match slot {
        Slot::Live { revision } => revision == base_revision,
        Slot::Vacant | Slot::Deleted { .. } => false,
    }
}
