//! The simulator's hidden ground truth. Nothing outside this crate can read
//! it: normal processing sees only what the simulated sensors report.
//! (A debug-truth view for the UI is a later, explicitly separate layer.)
//!
//! Integer millimetres throughout, so truth is bit-identical on every
//! platform.

/// A vehicle driving a rectangular loop at constant speed.
#[derive(Debug, Clone)]
pub(crate) struct LoopPath {
    /// Corners, counter-clockwise, in millimetres.
    corners: [(i64, i64); 4],
    speed_mm_per_tick: i64,
}

/// A building footprint that blocks the detector's view.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Occluder {
    x_mm: (i64, i64),
    y_mm: (i64, i64),
}

impl Occluder {
    pub(crate) fn hides(&self, (x, y): (i64, i64)) -> bool {
        self.x_mm.0 <= x && x <= self.x_mm.1 && self.y_mm.0 <= y && y <= self.y_mm.1
    }
}

/// The demo scenario's truth.
#[derive(Debug, Clone)]
pub(crate) struct Truth {
    pub(crate) vehicle: LoopPath,
    pub(crate) occluder: Occluder,
}

impl Truth {
    /// The bundled scenario: a vehicle lapping a 120 m × 80 m loop at
    /// 5 m/s (80 s per lap), passing behind a building on the north side
    /// for 8 s per lap.
    pub(crate) fn demo() -> Self {
        Self {
            vehicle: LoopPath {
                corners: [
                    (-60_000, -40_000),
                    (60_000, -40_000),
                    (60_000, 40_000),
                    (-60_000, 40_000),
                ],
                speed_mm_per_tick: 500,
            },
            occluder: Occluder {
                x_mm: (10_000, 50_000),
                y_mm: (30_000, 55_000),
            },
        }
    }
}

impl LoopPath {
    /// Position at `tick`. Every edge is axis-aligned, so positions are
    /// exact integers.
    pub(crate) fn position(&self, tick: u64) -> (i64, i64) {
        let edges: [((i64, i64), (i64, i64)); 4] = [
            (self.corners[0], self.corners[1]),
            (self.corners[1], self.corners[2]),
            (self.corners[2], self.corners[3]),
            (self.corners[3], self.corners[0]),
        ];
        let lengths = edges.map(|(a, b)| (b.0 - a.0).abs() + (b.1 - a.1).abs());
        let perimeter: i64 = lengths.iter().sum();
        let ticks_per_lap = u64::try_from(perimeter / self.speed_mm_per_tick)
            .unwrap_or(1)
            .max(1);
        let in_lap = i64::try_from(tick % ticks_per_lap).unwrap_or(0);
        let mut d = in_lap * self.speed_mm_per_tick;
        for ((a, b), len) in edges.into_iter().zip(lengths) {
            if d < len {
                return (
                    a.0 + (b.0 - a.0).signum() * d,
                    a.1 + (b.1 - a.1).signum() * d,
                );
            }
            d -= len;
        }
        self.corners[0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_positions() {
        let t = Truth::demo();
        let p = |tick| t.vehicle.position(tick);
        assert_eq!(p(0), (-60_000, -40_000));
        assert_eq!(p(1), (-59_500, -40_000));
        assert_eq!(p(240), (60_000, -40_000));
        assert_eq!(p(400), (60_000, 40_000));
        assert_eq!(p(640), (-60_000, 40_000));
        assert_eq!(p(799), (-60_000, -39_500));
        assert_eq!(p(800), p(0));
        assert_eq!(p(u64::MAX), p(u64::MAX % 800));
    }

    #[test]
    fn occluded_for_eight_seconds_per_lap() {
        let t = Truth::demo();
        let hidden = (0..800)
            .filter(|&tick| t.occluder.hides(t.vehicle.position(tick)))
            .count();
        assert_eq!(hidden, 81); // 40 m at 0.5 m per tick, both ends inclusive
    }
}
