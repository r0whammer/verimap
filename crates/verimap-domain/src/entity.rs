//! Entity tracks: what observations established about one detected object.

use std::collections::VecDeque;

use crate::{DomainError, ObjectId, ScenePoint, SourceId};

/// Scene time per clock tick, in milliseconds. The scene clock advances
/// only through committed clock events, never with the wall clock or with
/// rendering, so pausing pauses aging.
pub const TICK_MS: u64 = 100;

/// An entity with no observation for more than this many ticks (3 s) is
/// [`Freshness::Stale`].
pub const STALE_AFTER_TICKS: u64 = 30;

/// An entity with no observation for more than this many ticks (15 s) is
/// [`Freshness::Expired`]. Expired is not deleted: the track stays in the
/// scene until something deletes it explicitly.
pub const EXPIRE_AFTER_TICKS: u64 = 150;

/// What kind of object a track is believed to be. Detector output, not a
/// verified classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityClass {
    /// A person.
    Person,
    /// An animal.
    Animal,
    /// A vehicle.
    Vehicle,
    /// A building.
    Building,
    /// A static obstacle.
    Obstacle,
    /// A sensor platform.
    Sensor,
    /// Detected, class not determined.
    Unknown,
}

impl EntityClass {
    /// Every class, in canonical order.
    pub const ALL: [EntityClass; 7] = [
        Self::Person,
        Self::Animal,
        Self::Vehicle,
        Self::Building,
        Self::Obstacle,
        Self::Sensor,
        Self::Unknown,
    ];

    /// Stable lowercase name, used on the wire and in canonical encoding.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Animal => "animal",
            Self::Vehicle => "vehicle",
            Self::Building => "building",
            Self::Obstacle => "obstacle",
            Self::Sensor => "sensor",
            Self::Unknown => "unknown",
        }
    }
}

/// A detection or track confidence in basis points, `0..=10000`: the
/// detector's score, not a calibrated probability. Constructed only through
/// the kernel's proved guard (`verimap_kernel::Confidence::new`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Confidence(u16);

impl Confidence {
    /// Validates `basis_points` with the kernel guard.
    pub fn new(basis_points: u16) -> Result<Self, DomainError> {
        verimap_kernel::Confidence::new(basis_points)
            .map(|c| Self(c.basis_points()))
            .map_err(|e| DomainError::from_kernel("confidence", e))
    }

    /// The validated value in basis points.
    pub fn basis_points(self) -> u16 {
        self.0
    }
}

/// Horizontal position uncertainty: one standard deviation, in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uncertainty {
    sigma_mm: u32,
}

impl Uncertainty {
    /// Largest accepted σ: 100 m. A detection less certain than that does
    /// not locate anything on a 200 m site.
    pub const MAX_SIGMA_MM: u32 = 100_000;

    /// Accepts `1..=MAX_SIGMA_MM` millimetres. Zero would claim a perfect
    /// measurement, which no sensor provides.
    pub fn new(sigma_mm: u32) -> Result<Self, DomainError> {
        if sigma_mm == 0 || sigma_mm > Self::MAX_SIGMA_MM {
            return Err(DomainError::OutOfRange {
                field: "uncertainty.sigma",
            });
        }
        Ok(Self { sigma_mm })
    }

    /// One standard deviation, millimetres.
    pub fn sigma_mm(&self) -> u32 {
        self.sigma_mm
    }
}

/// Which observation a state came from: the source and its sequence.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObservationRef {
    /// Reporting source.
    pub source: SourceId,
    /// The source's observation sequence number.
    pub sequence: u64,
}

/// One accepted observation of a track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TrackFix {
    /// Acquisition time in scene ticks (the source's clock domain, which in
    /// this prototype is the scene clock).
    pub acquired_tick: u64,
    /// Measured position.
    pub position: ScenePoint,
    /// Horizontal uncertainty of `position`.
    pub uncertainty: Uncertainty,
    /// Detector confidence. A score, not a calibrated probability.
    pub confidence: Confidence,
}

/// How recently an entity was observed, from the scene clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Freshness {
    /// Observed within [`STALE_AFTER_TICKS`].
    Active,
    /// Not observed for more than [`STALE_AFTER_TICKS`].
    Stale,
    /// Not observed for more than [`EXPIRE_AFTER_TICKS`].
    Expired,
}

impl Freshness {
    /// Freshness at scene tick `now` of an entity last observed at `last`.
    /// An observation time after `now` cannot be committed, so the age is
    /// never negative; it saturates at 0 regardless.
    pub fn at(last_observed_tick: u64, now: u64) -> Self {
        let age = now.saturating_sub(last_observed_tick);
        if age > EXPIRE_AFTER_TICKS {
            Self::Expired
        } else if age > STALE_AFTER_TICKS {
            Self::Stale
        } else {
            Self::Active
        }
    }

    /// Stable lowercase name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Stale => "stale",
            Self::Expired => "expired",
        }
    }
}

/// An entity track: class, latest fix, and a bounded observation history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entity {
    id: ObjectId,
    class: EntityClass,
    latest: TrackFix,
    source: ObservationRef,
    history: VecDeque<HistoryEntry>,
}

/// One past fix of an entity and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    /// The fix.
    pub fix: TrackFix,
    /// Its source observation.
    pub source: ObservationRef,
    /// Sequence of the committed event that recorded it.
    pub revision: u64,
}

impl Entity {
    /// Most recent fixes kept per entity, newest last.
    pub const HISTORY_LEN: usize = 16;

    pub(crate) fn new(
        id: ObjectId,
        class: EntityClass,
        fix: TrackFix,
        source: ObservationRef,
        revision: u64,
    ) -> Self {
        let mut history = VecDeque::with_capacity(Self::HISTORY_LEN);
        history.push_back(HistoryEntry {
            fix,
            source: source.clone(),
            revision,
        });
        Self {
            id,
            class,
            latest: fix,
            source,
            history,
        }
    }

    pub(crate) fn observe(&mut self, fix: TrackFix, source: ObservationRef, revision: u64) {
        if self.history.len() >= Self::HISTORY_LEN {
            self.history.pop_front();
        }
        self.history.push_back(HistoryEntry {
            fix,
            source: source.clone(),
            revision,
        });
        self.latest = fix;
        self.source = source;
    }

    /// Stable track ID.
    pub fn id(&self) -> &ObjectId {
        &self.id
    }

    /// Believed class.
    pub fn class(&self) -> EntityClass {
        self.class
    }

    /// Latest accepted fix: position, uncertainty, confidence, time.
    pub fn latest(&self) -> &TrackFix {
        &self.latest
    }

    /// Source of the latest fix.
    pub fn source(&self) -> &ObservationRef {
        &self.source
    }

    /// Acquisition tick of the latest fix.
    pub fn last_observed_tick(&self) -> u64 {
        self.latest.acquired_tick
    }

    /// Freshness at scene tick `now`.
    pub fn freshness(&self, now: u64) -> Freshness {
        Freshness::at(self.latest.acquired_tick, now)
    }

    /// Up to [`Entity::HISTORY_LEN`] most recent fixes, oldest first.
    pub fn history(&self) -> impl ExactSizeIterator<Item = &HistoryEntry> {
        self.history.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freshness_thresholds_are_exclusive() {
        assert_eq!(Freshness::at(100, 100), Freshness::Active);
        assert_eq!(
            Freshness::at(100, 100 + STALE_AFTER_TICKS),
            Freshness::Active
        );
        assert_eq!(
            Freshness::at(100, 101 + STALE_AFTER_TICKS),
            Freshness::Stale
        );
        assert_eq!(
            Freshness::at(100, 100 + EXPIRE_AFTER_TICKS),
            Freshness::Stale
        );
        assert_eq!(
            Freshness::at(100, 101 + EXPIRE_AFTER_TICKS),
            Freshness::Expired
        );
        assert_eq!(Freshness::at(100, 0), Freshness::Active);
        assert_eq!(Freshness::at(0, u64::MAX), Freshness::Expired);
    }

    #[test]
    fn confidence_uses_the_kernel_bounds() {
        assert_eq!(
            Confidence::new(10_000).map(Confidence::basis_points),
            Ok(10_000)
        );
        assert_eq!(
            Confidence::new(10_001),
            Err(DomainError::ConfidenceOutOfRange {
                field: "confidence"
            })
        );
    }

    #[test]
    fn uncertainty_bounds() {
        assert!(Uncertainty::new(0).is_err());
        assert!(Uncertainty::new(1).is_ok());
        assert!(Uncertainty::new(Uncertainty::MAX_SIGMA_MM).is_ok());
        assert!(Uncertainty::new(Uncertainty::MAX_SIGMA_MM + 1).is_err());
    }

    #[test]
    fn history_is_bounded_and_newest_last() {
        let id = ObjectId::parse("id", "trk:1").unwrap();
        let source = ObservationRef {
            source: SourceId::new("s", "cam").unwrap(),
            sequence: 0,
        };
        let fix = |t: u64| TrackFix {
            acquired_tick: t,
            position: ScenePoint::new(0, 0, 0).unwrap(),
            uncertainty: Uncertainty::new(1).unwrap(),
            confidence: Confidence::new(1).unwrap(),
        };
        let mut e = Entity::new(id, EntityClass::Vehicle, fix(0), source.clone(), 1);
        for t in 1..40 {
            e.observe(fix(t), source.clone(), t + 1);
        }
        assert_eq!(e.history().len(), Entity::HISTORY_LEN);
        assert_eq!(e.history().last().map(|h| h.fix.acquired_tick), Some(39));
        assert_eq!(e.history().next().map(|h| h.fix.acquired_tick), Some(24));
        assert_eq!(e.last_observed_tick(), 39);
    }
}
