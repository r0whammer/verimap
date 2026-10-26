//! Deterministic synthetic scenes, simulated sensors, and the local session
//! that drives them through VeriMap's normal processing.
//!
//! The simulator owns the hidden ground truth of its scenes (the private
//! `truth` module). Normal processing receives only what a simulated sensor
//! reports and never reads truth.
//!
//! Determinism: a seed and a tick count fully determine every observation.
//! Randomness comes from SplitMix64, truth and noise are integer
//! millimetres, and scene time advances in explicit integer ticks.

#![forbid(unsafe_code)]

mod rng;
mod sensor;
mod session;
mod truth;

pub use sensor::{DETECTOR_SOURCE_ID, SCENE_CLOCK_DOMAIN};
pub use session::{Advanced, IngestStats, LocalSession};
use verimap_domain::{
    DomainError, GeodeticAnchor, ObservationEnvelope, SceneBounds, SiteFrame, SiteId,
};

/// Version of the bundled synthetic scene definitions. Bump it whenever a
/// scene changes, so recorded sessions can tell which scene they used.
pub const SCENE_VERSION: u32 = 2;

/// Version of the simulator's behavior (sensors, noise, delivery). Bump it
/// whenever the same seed would produce different observations.
pub const SIMULATOR_VERSION: u32 = 1;

/// Seed of the demo session the web client starts.
pub const DEMO_SEED: u64 = 1;

/// The bundled fictional demo site: a 200 m × 200 m outdoor area centred on
/// its anchor, from 20 m below to 80 m above it.
///
/// The anchor is an arbitrary rural point chosen only so that longitude and
/// latitude have distinct signs and magnitudes (a swap fails validation). It
/// has no survey meaning. Its height is 0 m above the WGS84 ellipsoid, so the
/// synthetic ground lies on the renderer's plain ellipsoid surface.
pub fn demo_site() -> Result<SiteFrame, DomainError> {
    SiteFrame::new(
        SiteId::new("demo-site-alpha")?,
        "Synthetic Site Alpha",
        GeodeticAnchor::new(-98.35, 38.75, 0.0)?,
        SceneBounds::new((-100_000, 100_000), (-100_000, 100_000), (-20_000, 80_000))?,
    )
}

/// The demo scenario's truth and sensors, stepped one tick at a time.
#[derive(Debug, Clone)]
pub struct Simulator {
    tick: u64,
    rng: rng::SplitMix64,
    truth: truth::Truth,
    detector: sensor::Detector,
    /// Observations taken but not yet delivered, with delivery ticks.
    in_flight: Vec<(u64, ObservationEnvelope)>,
}

impl Simulator {
    /// The demo scenario at tick 0.
    pub fn new(seed: u64) -> Self {
        Self {
            tick: 0,
            rng: rng::SplitMix64::new(seed),
            truth: truth::Truth::demo(),
            detector: sensor::Detector::demo(),
            in_flight: Vec::new(),
        }
    }

    /// Current simulation tick.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Advances one tick and returns the observations delivered at the new
    /// tick, in (delivery tick, source sequence) order. Delivery order can
    /// differ from acquisition order.
    pub fn step(&mut self) -> Vec<ObservationEnvelope> {
        self.tick += 1;
        if let Some(taken) = self.detector.sense(&self.truth, self.tick, &mut self.rng) {
            self.in_flight.push(taken);
        }
        let now = self.tick;
        let (mut due, waiting): (Vec<_>, Vec<_>) = self
            .in_flight
            .drain(..)
            .partition(|(deliver, _)| *deliver <= now);
        self.in_flight = waiting;
        due.sort_by_key(|(deliver, e)| (*deliver, e.sequence));
        due.into_iter().map(|(_, e)| e).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_site_is_valid_and_200m_square() {
        let site = demo_site().expect("bundled site must validate");
        let b = site.bounds();
        assert_eq!(b.x().max() - b.x().min(), 200_000);
        assert_eq!(b.y().max() - b.y().min(), 200_000);
        assert_eq!(site.anchor().ellipsoidal_height_m(), 0.0);
    }

    #[test]
    fn simulator_is_reproducible_and_delays_some_observations() {
        let run = |seed| {
            let mut s = Simulator::new(seed);
            (0..400).flat_map(|_| s.step()).collect::<Vec<_>>()
        };
        let a = run(1);
        assert_eq!(a, run(1));
        assert_ne!(a, run(2));
        assert!(a.iter().any(|e| e.received_tick > e.acquired_tick));
        assert!(a.iter().all(|e| e.received_tick >= e.acquired_tick));
        // Some deliveries arrive out of acquisition order.
        assert!(
            a.windows(2)
                .any(|w| w[1].acquired_tick < w[0].acquired_tick)
        );
    }
}
