//! Observations, and the normalization every source goes through before
//! the estimator sees it. Simulated and future physical sources share this
//! path; transport-specific parsing happens before it, outside the domain.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Confidence, DomainError, EntityClass, ObservationRef, SceneBounds, ScenePoint, SourceId,
    TrackFix, Uncertainty, metres_to_mm,
};

/// The scene frame itself. This prototype assumes a known platform pose,
/// so sources report directly in the scene frame (docs/coordinates.md).
pub const SCENE_FRAME_ID: &str = "scene";

/// What a source reported, before validation. Values are as received:
/// strings unchecked and floating point possibly non-finite.
#[derive(Debug, Clone, PartialEq)]
pub enum ObservationPayload {
    /// A detected object with a measured 3D position (for example a camera
    /// detection with depth). An image-space box without depth does not
    /// locate anything and is not this payload.
    Detection {
        /// Detector class.
        class: EntityClass,
        /// Position in `frame_id`, metres.
        position_m: [f64; 3],
        /// Horizontal 1σ uncertainty, metres.
        sigma_m: f64,
        /// Detector confidence, basis points.
        confidence_bp: u16,
    },
}

/// One observation as it arrives from a source.
#[derive(Debug, Clone, PartialEq)]
pub struct ObservationEnvelope {
    /// Source identifier.
    pub source_id: String,
    /// The source's session; sequences restart with each session.
    pub source_session: u32,
    /// The source's observation sequence within its session.
    pub sequence: u64,
    /// When the measurement was taken, in `clock_domain` ticks.
    pub acquired_tick: u64,
    /// Clock that `acquired_tick` belongs to.
    pub clock_domain: String,
    /// When the observation reached normalization, in scene ticks.
    pub received_tick: u64,
    /// Frame the payload's coordinates are in.
    pub frame_id: String,
    /// Version of the frame's calibration/transform.
    pub transform_version: u32,
    /// The measurement.
    pub payload: ObservationPayload,
}

/// What a source declares when it registers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRegistration {
    /// Source identifier.
    pub id: SourceId,
    /// Current source session.
    pub session: u32,
    /// The clock its acquisition times use. Only the scene clock domain is
    /// comparable with scene time in this prototype.
    pub clock_domain: String,
    /// Frame its payloads use.
    pub frame_id: String,
    /// Expected transform version of that frame.
    pub transform_version: u32,
}

/// A validated detection, ready for association.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    /// Where it came from.
    pub source: ObservationRef,
    /// Detector class.
    pub class: EntityClass,
    /// Quantized fix.
    pub fix: TrackFix,
}

/// Why an observation was rejected before reaching the estimator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservationError {
    /// A field failed validation.
    Invalid(DomainError),
    /// The source is not registered.
    UnknownSource,
    /// The envelope's source session is not the registered one.
    SessionMismatch,
    /// The acquisition clock is not the source's registered clock.
    ClockDomainMismatch,
    /// No transform from this frame into the scene frame is known.
    UnknownFrame,
    /// The frame's transform version differs from the registered one.
    TransformVersionMismatch,
    /// Received before it was acquired.
    ReceivedBeforeAcquired,
    /// This source sequence was already accepted.
    Duplicate,
    /// Older than the duplicate-detection window, so it cannot be told
    /// apart from a duplicate.
    OutsideWindow,
}

impl core::fmt::Display for ObservationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Invalid(e) => write!(f, "{e}"),
            Self::UnknownSource => write!(f, "unknown source"),
            Self::SessionMismatch => write!(f, "source session mismatch"),
            Self::ClockDomainMismatch => write!(f, "clock domain mismatch"),
            Self::UnknownFrame => write!(f, "unknown frame"),
            Self::TransformVersionMismatch => write!(f, "transform version mismatch"),
            Self::ReceivedBeforeAcquired => write!(f, "received before acquired"),
            Self::Duplicate => write!(f, "duplicate observation"),
            Self::OutsideWindow => write!(f, "observation older than the duplicate window"),
        }
    }
}

impl From<DomainError> for ObservationError {
    fn from(e: DomainError) -> Self {
        Self::Invalid(e)
    }
}

#[derive(Debug, Clone)]
struct SourceState {
    registration: SourceRegistration,
    /// Accepted sequences no older than `DEDUP_WINDOW` below the highest.
    recent: BTreeSet<u64>,
}

/// Validates observations from registered sources against the site.
#[derive(Debug, Clone)]
pub struct Normalizer {
    bounds: SceneBounds,
    sources: BTreeMap<SourceId, SourceState>,
}

impl Normalizer {
    /// Sequences this far below a source's highest accepted sequence are
    /// rejected, since duplicates can no longer be detected.
    pub const DEDUP_WINDOW: u64 = 4096;

    /// A normalizer for a site with these bounds.
    pub fn new(bounds: SceneBounds) -> Self {
        Self {
            bounds,
            sources: BTreeMap::new(),
        }
    }

    /// Registers (or re-registers, resetting duplicate detection) a source.
    pub fn register(&mut self, registration: SourceRegistration) {
        self.sources.insert(
            registration.id.clone(),
            SourceState {
                registration,
                recent: BTreeSet::new(),
            },
        );
    }

    /// Validates `env` completely; only an accepted observation updates
    /// duplicate detection.
    pub fn normalize(&mut self, env: &ObservationEnvelope) -> Result<Detection, ObservationError> {
        let id = SourceId::new("source_id", &env.source_id)?;
        let state = self
            .sources
            .get_mut(&id)
            .ok_or(ObservationError::UnknownSource)?;
        let reg = &state.registration;
        if env.source_session != reg.session {
            return Err(ObservationError::SessionMismatch);
        }
        if env.clock_domain != reg.clock_domain {
            return Err(ObservationError::ClockDomainMismatch);
        }
        if env.frame_id != SCENE_FRAME_ID || env.frame_id != reg.frame_id {
            return Err(ObservationError::UnknownFrame);
        }
        if env.transform_version != reg.transform_version {
            return Err(ObservationError::TransformVersionMismatch);
        }
        if env.received_tick < env.acquired_tick {
            return Err(ObservationError::ReceivedBeforeAcquired);
        }
        if state.recent.contains(&env.sequence) {
            return Err(ObservationError::Duplicate);
        }
        let highest = state.recent.last().copied();
        if highest.is_some_and(|h| env.sequence < h.saturating_sub(Self::DEDUP_WINDOW)) {
            return Err(ObservationError::OutsideWindow);
        }
        let ObservationPayload::Detection {
            class,
            position_m: [x, y, z],
            sigma_m,
            confidence_bp,
        } = env.payload;
        let position = ScenePoint::from_metres(x, y, z)?;
        if !self.bounds.contains(&position) {
            return Err(DomainError::OutsideSite { field: "position" }.into());
        }
        let sigma_mm = metres_to_mm("sigma", sigma_m)?;
        let uncertainty =
            Uncertainty::new(
                u32::try_from(sigma_mm).map_err(|_| DomainError::OutOfRange {
                    field: "uncertainty.sigma",
                })?,
            )?;
        let fix = TrackFix {
            acquired_tick: env.acquired_tick,
            position,
            uncertainty,
            confidence: Confidence::new(confidence_bp)?,
        };
        state.recent.insert(env.sequence);
        if let Some(h) = state.recent.last().copied() {
            let floor = h.saturating_sub(Self::DEDUP_WINDOW);
            state.recent = state.recent.split_off(&floor);
        }
        Ok(Detection {
            source: ObservationRef {
                source: id,
                sequence: env.sequence,
            },
            class,
            fix,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normalizer() -> Normalizer {
        let bounds =
            SceneBounds::new((-10_000, 10_000), (-10_000, 10_000), (-1_000, 1_000)).unwrap();
        let mut n = Normalizer::new(bounds);
        n.register(SourceRegistration {
            id: SourceId::new("s", "cam-1").unwrap(),
            session: 1,
            clock_domain: "scene".into(),
            frame_id: SCENE_FRAME_ID.into(),
            transform_version: 1,
        });
        n
    }

    fn envelope(sequence: u64) -> ObservationEnvelope {
        ObservationEnvelope {
            source_id: "cam-1".into(),
            source_session: 1,
            sequence,
            acquired_tick: 10,
            clock_domain: "scene".into(),
            received_tick: 11,
            frame_id: SCENE_FRAME_ID.into(),
            transform_version: 1,
            payload: ObservationPayload::Detection {
                class: EntityClass::Vehicle,
                position_m: [1.0, -2.0, 0.25],
                sigma_m: 1.5,
                confidence_bp: 9000,
            },
        }
    }

    fn with_payload(f: impl FnOnce(&mut [f64; 3], &mut f64, &mut u16)) -> ObservationEnvelope {
        let mut env = envelope(1);
        let ObservationPayload::Detection {
            position_m,
            sigma_m,
            confidence_bp,
            ..
        } = &mut env.payload;
        f(position_m, sigma_m, confidence_bp);
        env
    }

    #[test]
    fn accepts_and_quantizes() {
        let d = normalizer().normalize(&envelope(1)).unwrap();
        assert_eq!(d.fix.position, ScenePoint::new(1_000, -2_000, 250).unwrap());
        assert_eq!(d.fix.uncertainty.sigma_mm(), 1_500);
        assert_eq!(d.source.sequence, 1);
        assert_eq!(d.class, EntityClass::Vehicle);
    }

    #[test]
    fn rejects_envelope_problems_before_payload() {
        let mut n = normalizer();
        type Case = (fn(&mut ObservationEnvelope), ObservationError);
        let cases: [Case; 6] = [
            (
                |e| e.source_id = "cam-2".into(),
                ObservationError::UnknownSource,
            ),
            (|e| e.source_session = 2, ObservationError::SessionMismatch),
            (
                |e| e.clock_domain = "gps".into(),
                ObservationError::ClockDomainMismatch,
            ),
            (
                |e| e.frame_id = "camera".into(),
                ObservationError::UnknownFrame,
            ),
            (
                |e| e.transform_version = 2,
                ObservationError::TransformVersionMismatch,
            ),
            (
                |e| e.received_tick = 9,
                ObservationError::ReceivedBeforeAcquired,
            ),
        ];
        for (mutate, expected) in cases {
            let mut env = envelope(1);
            mutate(&mut env);
            assert_eq!(n.normalize(&env), Err(expected));
        }
        // None of the rejections counted as seen.
        assert!(n.normalize(&envelope(1)).is_ok());
    }

    #[test]
    fn rejects_bad_payload_values() {
        let mut n = normalizer();
        let bad = [
            with_payload(|p, _, _| p[0] = f64::NAN),
            with_payload(|p, _, _| p[1] = f64::INFINITY),
            with_payload(|p, _, _| p[0] = 10.001),
            with_payload(|_, s, _| *s = 0.0),
            with_payload(|_, s, _| *s = -1.0),
            with_payload(|_, s, _| *s = f64::NAN),
            with_payload(|_, s, _| *s = 100.001),
            with_payload(|_, _, c| *c = 10_001),
        ];
        for env in bad {
            assert!(
                matches!(n.normalize(&env), Err(ObservationError::Invalid(_))),
                "{env:?}"
            );
        }
        // Boundary values are accepted.
        assert!(
            n.normalize(&with_payload(|p, s, c| {
                p[0] = 10.0;
                *s = 0.001;
                *c = 10_000;
            }))
            .is_ok()
        );
    }

    #[test]
    fn detects_duplicates_and_old_sequences() {
        let mut n = normalizer();
        assert!(n.normalize(&envelope(5)).is_ok());
        assert_eq!(n.normalize(&envelope(5)), Err(ObservationError::Duplicate));
        // Reordered but new: accepted.
        assert!(n.normalize(&envelope(3)).is_ok());
        let far = Normalizer::DEDUP_WINDOW + 10;
        assert!(n.normalize(&envelope(far)).is_ok());
        assert_eq!(
            n.normalize(&envelope(3)),
            Err(ObservationError::OutsideWindow)
        );
        assert!(n.normalize(&envelope(10)).is_ok());
    }
}
