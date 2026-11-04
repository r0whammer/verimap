//! Simulated platform pose. The platform path is known: this source reports
//! it, with optional noise, gaps, and delivery delay. It does not estimate
//! the path, and it never hands the truth module to the normalizer.

use verimap_domain::{
    ADAPTER_INTERFACE_VERSION, AdapterPhase, ObservationEnvelope, ObservationPayload,
    SourceCapabilities, SourceHealth,
};

use crate::rng::SplitMix64;
use crate::sensor::SCENE_CLOCK_DOMAIN;
use crate::truth::Truth;

/// Source id of the simulated pose adapter.
pub const POSE_SOURCE_ID: &str = "sim-pose-1";

/// Reports the platform pose in the scene frame.
///
/// Randomness comes from a private generator so this source does not change
/// the detector's sequence. Gaps are deterministic: every
/// [`PoseSource::GAP_EVERY`] scheduled tick is missed.
#[derive(Debug, Clone)]
pub(crate) struct PoseSource {
    phase: AdapterPhase,
    /// Ticks between scheduled acquisitions.
    period_ticks: u64,
    /// Noise half-width, millimetres. Zero is a perfect report.
    noise_half_width_mm: u32,
    /// Delivery delay is uniform in `0..=max_delay_ticks`.
    max_delay_ticks: u64,
    next_sequence: u64,
    /// The latest scheduled tick was a gap.
    last_was_gap: bool,
    rng: SplitMix64,
}

impl PoseSource {
    /// A gap on every tick whose index is a multiple of this.
    pub(crate) const GAP_EVERY: u64 = 5;

    /// Demo pose source: 0.2 m noise, up to 2 ticks of delay, a gap every
    /// fifth tick. The seed is mixed so it is not the detector's seed.
    pub(crate) fn demo(seed: u64) -> Self {
        Self {
            phase: AdapterPhase::Running,
            period_ticks: 1,
            noise_half_width_mm: 200,
            max_delay_ticks: 2,
            next_sequence: 1,
            last_was_gap: false,
            rng: SplitMix64::new(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15)),
        }
    }

    // The demo leaves the adapter running; tests cover the rest of the contract.
    #[allow(dead_code)]
    pub(crate) fn interface_version(&self) -> u32 {
        ADAPTER_INTERFACE_VERSION
    }

    #[allow(dead_code)]
    pub(crate) fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities {
            reports_pose: true,
            reports_detections: false,
            reports_ranges: false,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn phase(&self) -> AdapterPhase {
        self.phase
    }

    pub(crate) fn health(&self) -> SourceHealth {
        match self.phase {
            AdapterPhase::Stopped => SourceHealth::Unavailable,
            AdapterPhase::Running if self.last_was_gap => SourceHealth::Degraded,
            AdapterPhase::Running => SourceHealth::Healthy,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn start(&mut self) {
        self.phase = AdapterPhase::Running;
    }

    #[allow(dead_code)]
    pub(crate) fn stop(&mut self) {
        self.phase = AdapterPhase::Stopped;
    }

    /// The pose acquired at `tick`, if this tick is scheduled and not a gap,
    /// paired with the tick it should be delivered.
    pub(crate) fn acquire(
        &mut self,
        truth: &Truth,
        tick: u64,
    ) -> Option<(u64, ObservationEnvelope)> {
        if self.phase != AdapterPhase::Running || !tick.is_multiple_of(self.period_ticks) {
            return None;
        }
        // Consume the random draws in a fixed order even on a gap, so a gap
        // does not shift later noise.
        let dx = self.rng.symmetric(self.noise_half_width_mm);
        let dy = self.rng.symmetric(self.noise_half_width_mm);
        let dz = self.rng.symmetric(self.noise_half_width_mm);
        let delay = self.rng.below(self.max_delay_ticks + 1);
        if tick.is_multiple_of(Self::GAP_EVERY) {
            self.last_was_gap = true;
            return None;
        }
        self.last_was_gap = false;
        let (x, y) = truth.platform_position_mm(tick);
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        let m = |mm: i64| mm as f64 / 1000.0;
        Some((
            tick + delay,
            ObservationEnvelope {
                source_id: POSE_SOURCE_ID.into(),
                source_session: 1,
                sequence,
                acquired_tick: tick,
                clock_domain: SCENE_CLOCK_DOMAIN.into(),
                received_tick: tick + delay,
                frame_id: verimap_domain::SCENE_FRAME_ID.into(),
                transform_version: 1,
                payload: ObservationPayload::Pose {
                    position_m: [m(x + dx), m(y + dy), m(dz)],
                    sigma_m: 0.2,
                },
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaps_delays_and_stop_are_explicit() {
        let mut pose = PoseSource::demo(1);
        assert_eq!(pose.interface_version(), 1);
        assert!(pose.capabilities().reports_pose);
        assert!(!pose.capabilities().reports_detections);
        assert_eq!(pose.health(), SourceHealth::Healthy);

        let truth = Truth::demo();
        let mut delivered = Vec::new();
        for tick in 1..=10 {
            if let Some((deliver, env)) = pose.acquire(&truth, tick) {
                assert!(deliver >= tick);
                assert_eq!(env.acquired_tick, tick);
                assert_eq!(env.received_tick, deliver);
                delivered.push((deliver, env));
            }
        }
        // Tick 5 and 10 are gaps.
        let acquired: Vec<u64> = delivered.iter().map(|(_, e)| e.acquired_tick).collect();
        assert!(!acquired.contains(&5));
        assert!(!acquired.contains(&10));
        assert!(acquired.contains(&4) && acquired.contains(&6));
        assert_eq!(pose.health(), SourceHealth::Degraded);
        assert!(delivered.iter().any(|(d, e)| *d > e.acquired_tick));

        pose.stop();
        assert_eq!(pose.phase(), AdapterPhase::Stopped);
        assert_eq!(pose.health(), SourceHealth::Unavailable);
        assert!(pose.acquire(&truth, 11).is_none());
        pose.start();
        assert!(pose.acquire(&truth, 11).is_some());
        assert_eq!(pose.health(), SourceHealth::Healthy);
    }

    #[test]
    fn the_same_seed_repeats_and_a_different_seed_does_not() {
        let run = |seed| {
            let mut pose = PoseSource::demo(seed);
            let truth = Truth::demo();
            (1..=8)
                .filter_map(|tick| pose.acquire(&truth, tick))
                .collect::<Vec<_>>()
        };
        assert_eq!(run(1), run(1));
        assert_ne!(run(1), run(2));
    }

    #[test]
    fn an_off_period_tick_reports_nothing() {
        let mut pose = PoseSource::demo(1);
        pose.period_ticks = 2;
        let truth = Truth::demo();
        assert!(pose.acquire(&truth, 1).is_none());
        assert!(pose.acquire(&truth, 2).is_some());
    }
}
