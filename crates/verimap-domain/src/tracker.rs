//! The baseline tracker: deterministic nearest-neighbour association of
//! detections to existing tracks, gated by class, time, and distance.
//!
//! This is a baseline, not robust multi-sensor identity resolution. Two
//! same-class objects that pass close to each other can swap track IDs, and
//! an object unseen for longer than the coasting limit gets a new track.

use crate::{Detection, ObjectId, SceneStore};

/// Association gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackerConfig {
    /// Distance gate at zero elapsed time, millimetres.
    pub gate_base_mm: i64,
    /// Gate growth per tick of elapsed time, millimetres (the fastest
    /// speed the tracker expects, 10 m/s by default).
    pub gate_mm_per_tick: i64,
    /// Tracks unobserved for longer than this many ticks are not
    /// candidates; a later detection starts a new track.
    pub max_coast_ticks: u64,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            gate_base_mm: 5_000,
            gate_mm_per_tick: 1_000,
            max_coast_ticks: crate::EXPIRE_AFTER_TICKS,
        }
    }
}

/// The tracker's decision for one detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Association {
    /// Update this track.
    Existing(ObjectId),
    /// No track within the gates: start one.
    New,
    /// The best track already has a newer fix than this detection. The
    /// detection must not overwrite it, so it is not applied.
    Late(ObjectId),
}

/// Associates `detection` with the live tracks in `store`.
///
/// Rule: the candidates are live tracks of the same class whose latest fix
/// is at most `max_coast_ticks` from the detection, in either direction,
/// and whose horizontal distance is within
/// `gate_base_mm + gate_mm_per_tick × |Δt|`. The nearest candidate wins;
/// equal distances go to the smallest track ID. All arithmetic is integer,
/// so the result is identical on every platform.
pub fn associate(config: &TrackerConfig, store: &SceneStore, detection: &Detection) -> Association {
    let t = detection.fix.acquired_tick;
    let mut best: Option<(i128, &ObjectId, u64)> = None;
    for (_, entity) in store.entities() {
        if entity.class() != detection.class {
            continue;
        }
        let last = entity.last_observed_tick();
        let dt = t.abs_diff(last);
        if dt > config.max_coast_ticks {
            continue;
        }
        let gate =
            i128::from(config.gate_base_mm) + i128::from(config.gate_mm_per_tick) * i128::from(dt);
        let d2 = entity
            .latest()
            .position
            .horizontal_distance_sq(&detection.fix.position);
        if d2 > gate * gate {
            continue;
        }
        // Entities iterate in ID order, so a strict comparison keeps the
        // smallest ID among equal distances.
        if best.is_none_or(|(b, _, _)| d2 < b) {
            best = Some((d2, entity.id(), last));
        }
    }
    match best {
        None => Association::New,
        Some((_, id, last)) if last > t => Association::Late(id.clone()),
        Some((_, id, _)) => Association::Existing(id.clone()),
    }
}
