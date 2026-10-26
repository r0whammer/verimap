//! Simulated sensors. A sensor sees truth and reports observations with
//! noise, dropouts, and delivery delay. Its output enters the same
//! normalization path a physical source would.

use verimap_domain::{EntityClass, ObservationEnvelope, ObservationPayload, SCENE_FRAME_ID};

use crate::rng::SplitMix64;
use crate::truth::Truth;

/// The simulated detector's source ID.
pub const DETECTOR_SOURCE_ID: &str = "sim-detector-1";
/// Clock domain of simulated acquisition times: the scene clock itself.
pub const SCENE_CLOCK_DOMAIN: &str = "scene";

/// A synthetic "camera with depth" detector: it reports the vehicle's
/// position in the scene frame (known platform pose is an explicit
/// assumption of this milestone). This is synthetic detection output, not a
/// vision model.
#[derive(Debug, Clone)]
pub(crate) struct Detector {
    /// Ticks between frames.
    pub(crate) period_ticks: u64,
    /// Probability of missing a visible object, in percent.
    pub(crate) dropout_percent: u64,
    /// Horizontal noise is uniform in ±this, millimetres (1.5 m σ × √3).
    pub(crate) horizontal_half_width_mm: u32,
    /// Vertical noise is uniform in ±this, millimetres.
    pub(crate) vertical_half_width_mm: u32,
    /// Reported 1σ, metres.
    pub(crate) reported_sigma_m: f64,
    /// Delivery delay is uniform in `0..=max_delay_ticks`.
    pub(crate) max_delay_ticks: u64,
    next_sequence: u64,
}

impl Detector {
    pub(crate) fn demo() -> Self {
        Self {
            period_ticks: 2,
            dropout_percent: 8,
            horizontal_half_width_mm: 2_598,
            vertical_half_width_mm: 300,
            reported_sigma_m: 1.5,
            max_delay_ticks: 3,
            next_sequence: 1,
        }
    }

    /// The observation taken at `tick`, if any, and its delivery tick.
    /// Consumes random numbers in a fixed order whatever the outcome.
    pub(crate) fn sense(
        &mut self,
        truth: &Truth,
        tick: u64,
        rng: &mut SplitMix64,
    ) -> Option<(u64, ObservationEnvelope)> {
        if !tick.is_multiple_of(self.period_ticks) {
            return None;
        }
        let (x, y) = truth.vehicle.position(tick);
        let missed = rng.below(100) < self.dropout_percent;
        let dx = rng.symmetric(self.horizontal_half_width_mm);
        let dy = rng.symmetric(self.horizontal_half_width_mm);
        let dz = rng.symmetric(self.vertical_half_width_mm);
        let confidence = 7_000 + rng.below(2_501);
        let delay = rng.below(self.max_delay_ticks + 1);
        if missed || truth.occluder.hides((x, y)) {
            return None;
        }
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        let deliver = tick + delay;
        // Sensors report metres; the normalizer quantizes back to mm.
        let m = |mm: i64| mm as f64 / 1000.0;
        Some((
            deliver,
            ObservationEnvelope {
                source_id: DETECTOR_SOURCE_ID.into(),
                source_session: 1,
                sequence,
                acquired_tick: tick,
                clock_domain: SCENE_CLOCK_DOMAIN.into(),
                received_tick: deliver,
                frame_id: SCENE_FRAME_ID.into(),
                transform_version: 1,
                payload: ObservationPayload::Detection {
                    class: EntityClass::Vehicle,
                    position_m: [m(x + dx), m(y + dy), m(dz)],
                    sigma_m: self.reported_sigma_m,
                    confidence_bp: u16::try_from(confidence).unwrap_or(u16::MAX),
                },
            },
        ))
    }
}
