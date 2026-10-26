//! A local session: simulator → normalization → tracker → authority, one
//! scene tick at a time. The browser worker and native tests run exactly
//! this code.

use verimap_domain::{
    Association, Authority, Changes, Command, CommandError, DomainError, Normalizer,
    ObservationEnvelope, ObservationError, SCENE_FRAME_ID, SourceId, SourceRegistration,
    TrackTarget, TrackerConfig, associate,
};

use crate::Simulator;
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
}

impl LocalSession {
    /// Scene epoch of a new local session.
    pub const EPOCH: u32 = 1;

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
        Ok(Self {
            seed,
            sim: Simulator::new(seed),
            normalizer,
            tracker: TrackerConfig::default(),
            authority: Authority::new(site, Self::EPOCH, journal_limit),
            stats: IngestStats::default(),
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

    /// Observation counts so far.
    pub fn stats(&self) -> IngestStats {
        self.stats
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
        let detection = match self.normalizer.normalize(envelope) {
            Ok(d) => d,
            Err(ObservationError::Duplicate) => {
                self.stats.duplicates += 1;
                return Ok(Changes::default());
            }
            Err(_) => {
                self.stats.rejected += 1;
                return Ok(Changes::default());
            }
        };
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
