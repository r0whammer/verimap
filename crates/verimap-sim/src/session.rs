//! A local session: simulator → normalization → tracker → authority, one
//! scene tick at a time. The browser worker and native tests run exactly
//! this code.

use verimap_domain::{
    AcceptedObservation, Association, Authority, Changes, Command, CommandError, DomainError,
    Normalizer, ObservationEnvelope, ObservationError, PlatformPose, SCENE_FRAME_ID, SourceHealth,
    SourceId, SourceRegistration, TrackTarget, TrackerConfig, associate,
};

use crate::Simulator;
use crate::pose::POSE_SOURCE_ID;
use crate::sensor::{DETECTOR_SOURCE_ID, SCENE_CLOCK_DOMAIN};

/// What happened to the observations this session received.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IngestStats {
    /// Observations delivered by sources.
    pub received: u64,
    /// Turned into committed track events.
    pub accepted: u64,
    /// Rejected by normalization (invalid, unknown frame, ...).
    pub rejected: u64,
    /// Exact duplicates of accepted observations.
    pub duplicates: u64,
    /// Older than the matched track's latest fix; kept out of state.
    pub late: u64,
    /// Accepted platform poses. They are not scene events.
    pub poses: u64,
}

/// The result of [`LocalSession::advance`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advanced {
    /// Everything that changed, including before a stop.
    pub changes: Changes,
    /// Why the session stopped early, if it did.
    pub stopped: Option<CommandError>,
}

impl Advanced {
    /// The changes, or the error that stopped the advance.
    pub fn ok(self) -> Result<Changes, CommandError> {
        match self.stopped {
            None => Ok(self.changes),
            Some(e) => Err(e),
        }
    }
}

/// One simulated session with its own authority and journal.
#[derive(Debug, Clone)]
pub struct LocalSession {
    seed: u64,
    sim: Simulator,
    normalizer: Normalizer,
    tracker: TrackerConfig,
    authority: Authority,
    stats: IngestStats,
    /// Accepted poses, oldest first, capped so a long run stays bounded.
    poses: Vec<PlatformPose>,
}

impl LocalSession {
    /// Scene epoch of a new local session.
    pub const EPOCH: u32 = 1;
    /// Retained accepted poses. Older ones are dropped.
    const POSE_LOG_LIMIT: usize = 128;

    /// A session of the demo scenario with `seed` and the default journal
    /// bound.
    pub fn new(seed: u64) -> Result<Self, DomainError> {
        Self::with_journal_limit(seed, Authority::DEFAULT_JOURNAL_LIMIT)
    }

    /// A session with an explicit journal bound, in events.
    pub fn with_journal_limit(seed: u64, journal_limit: usize) -> Result<Self, DomainError> {
        let site = crate::demo_site()?;
        let mut normalizer = Normalizer::new(*site.bounds());
        normalizer.register(SourceRegistration {
            id: SourceId::new("source_id", DETECTOR_SOURCE_ID)?,
            session: 1,
            clock_domain: SCENE_CLOCK_DOMAIN.into(),
            frame_id: SCENE_FRAME_ID.into(),
            transform_version: 1,
        });
        normalizer.register(SourceRegistration {
            id: SourceId::new("source_id", POSE_SOURCE_ID)?,
            session: 1,
            clock_domain: SCENE_CLOCK_DOMAIN.into(),
            frame_id: SCENE_FRAME_ID.into(),
            transform_version: 1,
        });
        Ok(Self {
            seed,
            sim: Simulator::new(seed),
            normalizer,
            tracker: TrackerConfig::default(),
            authority: Authority::new(site, Self::EPOCH, journal_limit),
            stats: IngestStats::default(),
            poses: Vec::new(),
        })
    }

    /// The seed this session was started with.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The authority: live state and journal.
    pub fn authority(&self) -> &Authority {
        &self.authority
    }

    /// The writer. The UI does not call this; the wasm tests commit a chunk
    /// the bridge does not expose yet, so the view's chunk delta is covered.
    #[doc(hidden)]
    pub fn authority_mut(&mut self) -> &mut Authority {
        &mut self.authority
    }

    /// Observation counts so far.
    pub fn stats(&self) -> IngestStats {
        self.stats
    }

    /// Accepted platform poses still retained, oldest first.
    pub fn poses(&self) -> &[PlatformPose] {
        &self.poses
    }

    /// Health of the simulated pose adapter.
    pub fn pose_health(&self) -> SourceHealth {
        self.sim.pose_health()
    }

    /// Advances the simulation by `ticks` scene ticks. Each tick commits a
    /// clock event, then the events for the observations delivered at that
    /// tick, so the journal does not depend on how ticks are batched.
    ///
    /// Stops at the first error and reports it with the changes made
    /// before it. A full journal stops before the simulator moves, so no
    /// observation is silently lost.
    pub fn advance(&mut self, ticks: u32) -> Advanced {
        let mut changes = Changes::default();
        for _ in 0..ticks {
            if let Err(e) = self.advance_one(&mut changes) {
                return Advanced {
                    changes,
                    stopped: Some(e),
                };
            }
        }
        Advanced {
            changes,
            stopped: None,
        }
    }

    fn advance_one(&mut self, changes: &mut Changes) -> Result<(), CommandError> {
        let limit = self.authority.journal_limit();
        if self.authority.journal().len() >= limit {
            return Err(CommandError::JournalFull { limit });
        }
        let delivered = self.sim.step();
        changes.merge(self.authority.submit(Command::AdvanceClock {
            to_tick: self.sim.tick(),
        })?);
        for envelope in &delivered {
            changes.merge(self.ingest(envelope)?);
        }
        Ok(())
    }

    /// Submits an operator command.
    pub fn submit(&mut self, command: Command) -> Result<Changes, CommandError> {
        self.authority.submit(command)
    }

    fn ingest(&mut self, envelope: &ObservationEnvelope) -> Result<Changes, CommandError> {
        self.stats.received += 1;
        let accepted = match self.normalizer.normalize(envelope) {
            Ok(AcceptedObservation::Pose(pose)) => {
                // A pose is not scene state. Keep a bounded log for inspection.
                self.stats.poses += 1;
                self.poses.push(pose);
                let overflow = self.poses.len().saturating_sub(Self::POSE_LOG_LIMIT);
                self.poses.drain(..overflow);
                return Ok(Changes::default());
            }
            Ok(AcceptedObservation::Detection(d)) => d,
            Err(ObservationError::Duplicate) => {
                self.stats.duplicates += 1;
                return Ok(Changes::default());
            }
            Err(_) => {
                self.stats.rejected += 1;
                return Ok(Changes::default());
            }
        };
        let detection = accepted;
        let target = match associate(&self.tracker, self.authority.store(), &detection) {
            Association::Late(_) => {
                self.stats.late += 1;
                return Ok(Changes::default());
            }
            Association::New => TrackTarget::New,
            Association::Existing(id) => TrackTarget::Existing(id),
        };
        match self.authority.submit(Command::ObserveEntity {
            target,
            class: detection.class,
            fix: detection.fix,
            source: detection.source,
        }) {
            Ok(changes) => {
                self.stats.accepted += 1;
                Ok(changes)
            }
            // The reducer refused this observation (for example a late fix
            // the tracker did not catch); state is unchanged.
            Err(CommandError::Rejected(_)) => {
                self.stats.rejected += 1;
                Ok(Changes::default())
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use verimap_domain::{EntityClass, ObservationPayload};

    fn envelope(sequence: u64, acquired: u64, class: EntityClass) -> ObservationEnvelope {
        ObservationEnvelope {
            source_id: DETECTOR_SOURCE_ID.into(),
            source_session: 1,
            sequence,
            acquired_tick: acquired,
            clock_domain: SCENE_CLOCK_DOMAIN.into(),
            received_tick: acquired,
            frame_id: SCENE_FRAME_ID.into(),
            transform_version: 1,
            payload: ObservationPayload::Detection {
                class,
                position_m: [0.0, 0.0, 0.0],
                sigma_m: 1.5,
                confidence_bp: 8_000,
            },
        }
    }

    #[test]
    fn ingest_counts_duplicates_rejections_and_reducer_refusals() {
        let mut s = LocalSession::new(1).unwrap();
        let first = envelope(1, 0, EntityClass::Vehicle);
        assert!(s.ingest(&first).unwrap().entities.len() == 1);
        assert_eq!(s.stats().accepted, 1);
        assert!(s.ingest(&first).unwrap().is_empty());
        assert_eq!(s.stats().duplicates, 1);

        let mut bad = envelope(2, 0, EntityClass::Vehicle);
        bad.frame_id = "nowhere".into();
        assert!(s.ingest(&bad).unwrap().is_empty());
        assert_eq!(s.stats().rejected, 1);

        // Acquired after the scene clock: the tracker would start a track,
        // and the reducer refuses it. State does not change.
        let future = envelope(3, 5, EntityClass::Person);
        assert!(s.ingest(&future).unwrap().is_empty());
        assert_eq!(s.stats().rejected, 2);
        assert_eq!(s.authority().store().entity_record_count(), 1);
    }

    #[test]
    fn poses_have_gaps_and_do_not_enter_the_journal() {
        let mut batched = LocalSession::new(1).unwrap();
        batched.advance(200);
        let mut stepped = LocalSession::new(1).unwrap();
        for _ in 0..200 {
            stepped.advance(1);
        }
        assert_eq!(batched.authority().journal(), stepped.authority().journal());
        assert_eq!(batched.poses(), stepped.poses());
        assert_eq!(batched.poses().len(), LocalSession::POSE_LOG_LIMIT);
        let acquired: Vec<u64> = batched.poses().iter().map(|p| p.acquired_tick).collect();
        assert!(acquired.iter().all(|tick| !tick.is_multiple_of(5)));
        assert!(batched.authority().journal().iter().all(|event| {
            matches!(
                event.payload,
                verimap_domain::EventPayload::ClockAdvanced { .. }
                    | verimap_domain::EventPayload::EntityObserved { .. }
            )
        }));
        assert_eq!(
            batched.pose_health(),
            verimap_domain::SourceHealth::Degraded
        );
    }

    #[test]
    fn stopped_advance_is_an_error() {
        let stopped = Advanced {
            changes: Changes::default(),
            stopped: Some(CommandError::JournalFull { limit: 1 }),
        };
        assert!(matches!(
            stopped.ok(),
            Err(CommandError::JournalFull { limit: 1 })
        ));
        let done = Advanced {
            changes: Changes::default(),
            stopped: None,
        };
        assert!(done.ok().unwrap().is_empty());
    }
}
