//! The authority: turns commands into committed events and keeps the
//! journal. There is exactly one authority per session; in local mode it
//! lives in the browser worker (or the CLI).

use verimap_kernel::{Slot, base_revision_matches, next_sequence};

use crate::{
    Annotation, ApplyError, Changes, CommittedEvent, DomainError, EntityClass, EventPayload,
    ObjectId, ObservationRef, Placement, Provenance, ScenePoint, SceneStore, SiteFrame, TrackFix,
};

/// Which track an observation belongs to, as the estimator decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackTarget {
    /// Start a new track; the authority allocates its ID.
    New,
    /// Update this existing track.
    Existing(ObjectId),
}

/// A requested change. Commands are proposals: the authority validates each
/// one fully, then either commits exactly one event or changes nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Advance the scene clock to `to_tick`.
    AdvanceClock {
        /// New scene time, in ticks; must be later than the current tick.
        to_tick: u64,
    },
    /// Record an accepted observation on a track.
    ObserveEntity {
        /// Track to update or create.
        target: TrackTarget,
        /// Observed class.
        class: EntityClass,
        /// The fix.
        fix: TrackFix,
        /// Source observation.
        source: ObservationRef,
    },
    /// Delete a track explicitly, if it is unchanged since `base_revision`.
    RemoveEntity {
        /// Track ID.
        id: ObjectId,
        /// Revision the operator saw.
        base_revision: u64,
    },
    /// Place a new marker; the authority allocates its ID.
    AddMarker {
        /// Display label.
        label: String,
        /// Position; its height follows `placement`.
        position: ScenePoint,
        /// Where the height came from.
        placement: Placement,
    },
    /// Change a marker's label and/or position, if it is unchanged since
    /// `base_revision`.
    EditMarker {
        /// Annotation ID.
        id: ObjectId,
        /// Revision the operator saw.
        base_revision: u64,
        /// New label, if changing.
        label: Option<String>,
        /// New position and placement, if moving.
        position: Option<(ScenePoint, Placement)>,
    },
    /// Delete an annotation, if it is unchanged since `base_revision`.
    DeleteAnnotation {
        /// Annotation ID.
        id: ObjectId,
        /// Revision the operator saw.
        base_revision: u64,
    },
}

/// Why a command was not committed. Nothing changes when `submit` fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    /// A value failed validation.
    Invalid(DomainError),
    /// No object with this ID exists in the epoch.
    NotFound {
        /// Object ID.
        id: ObjectId,
    },
    /// The object was deleted.
    Deleted {
        /// Object ID.
        id: ObjectId,
    },
    /// The object changed since the revision the edit was based on.
    PreconditionFailed {
        /// Object ID.
        id: ObjectId,
        /// Revision the edit was based on.
        base_revision: u64,
        /// The object's current revision.
        current: u64,
    },
    /// An edit that changes nothing.
    NothingToChange,
    /// The journal reached its configured limit; no further event can be
    /// recorded in this session.
    JournalFull {
        /// The limit, in events.
        limit: usize,
    },
    /// The authority sequence is exhausted for this epoch.
    SequenceExhausted,
    /// No free ID in the namespace (cannot happen below `usize::MAX` objects).
    IdSpaceExhausted,
    /// The reducer refused the event.
    Rejected(ApplyError),
}

impl core::fmt::Display for CommandError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Invalid(e) => write!(f, "{e}"),
            Self::NotFound { id } => write!(f, "{id} does not exist"),
            Self::Deleted { id } => write!(f, "{id} was deleted"),
            Self::PreconditionFailed {
                id,
                base_revision,
                current,
            } => write!(
                f,
                "{id} changed since revision {base_revision} (now {current}); reload and retry"
            ),
            Self::NothingToChange => write!(f, "the edit changes nothing"),
            Self::JournalFull { limit } => write!(
                f,
                "the session journal is full ({limit} events); start a new session to continue"
            ),
            Self::SequenceExhausted => write!(f, "the event sequence is exhausted"),
            Self::IdSpaceExhausted => write!(f, "no free object ID"),
            Self::Rejected(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CommandError {}

impl From<DomainError> for CommandError {
    fn from(e: DomainError) -> Self {
        Self::Invalid(e)
    }
}

/// The single writer of a session: the store and its committed journal.
#[derive(Debug, Clone)]
pub struct Authority {
    store: SceneStore,
    journal: Vec<CommittedEvent>,
    journal_limit: usize,
}

/// ID namespace of tracks created by the baseline tracker.
pub const TRACK_NAMESPACE: &str = "trk";
/// ID namespace of operator annotations.
pub const OPERATOR_NAMESPACE: &str = "op";

impl Authority {
    /// Default journal bound: about 4 hours of the demo scenario.
    pub const DEFAULT_JOURNAL_LIMIT: usize = 200_000;

    /// A new session at tick 0 with an empty journal.
    pub fn new(site: SiteFrame, epoch: u32, journal_limit: usize) -> Self {
        Self {
            store: SceneStore::new(site, epoch),
            journal: Vec::new(),
            journal_limit,
        }
    }

    /// Current live state.
    pub fn store(&self) -> &SceneStore {
        &self.store
    }

    /// Every committed event, in sequence.
    pub fn journal(&self) -> &[CommittedEvent] {
        &self.journal
    }

    /// The configured journal bound, in events.
    pub fn journal_limit(&self) -> usize {
        self.journal_limit
    }

    /// Validates `command`, commits one event, and applies it, or changes
    /// nothing and returns why.
    pub fn submit(&mut self, command: Command) -> Result<Changes, CommandError> {
        if self.journal.len() >= self.journal_limit {
            return Err(CommandError::JournalFull {
                limit: self.journal_limit,
            });
        }
        let sequence = next_sequence(self.store.last_sequence())
            .map_err(|_| CommandError::SequenceExhausted)?;
        let store = &self.store;
        let tick = store.tick();
        let (tick, provenance, payload) = match command {
            Command::AdvanceClock { to_tick } => (
                to_tick,
                Provenance::Clock,
                EventPayload::ClockAdvanced { to_tick },
            ),
            Command::ObserveEntity {
                target,
                class,
                fix,
                source,
            } => {
                let id = match target {
                    TrackTarget::New => {
                        allocate(TRACK_NAMESPACE, store.entity_record_count(), |id| {
                            store.entity_slot(id)
                        })?
                    }
                    TrackTarget::Existing(id) => id,
                };
                (
                    tick,
                    Provenance::Observation(source),
                    EventPayload::EntityObserved { id, class, fix },
                )
            }
            Command::RemoveEntity { id, base_revision } => {
                check_base(&id, store.entity_slot(&id), base_revision)?;
                (
                    tick,
                    Provenance::Operator,
                    EventPayload::EntityRemoved { id },
                )
            }
            Command::AddMarker {
                label,
                position,
                placement,
            } => {
                let id = allocate(OPERATOR_NAMESPACE, store.annotation_record_count(), |id| {
                    store.annotation_slot(id)
                })?;
                let annotation = Annotation::marker(id, &label, position, placement)?;
                (
                    tick,
                    Provenance::Operator,
                    EventPayload::AnnotationPut { annotation },
                )
            }
            Command::EditMarker {
                id,
                base_revision,
                label,
                position,
            } => {
                check_base(&id, store.annotation_slot(&id), base_revision)?;
                let Some((_, current)) = store.annotation(&id) else {
                    return Err(CommandError::NotFound { id });
                };
                let label = label.unwrap_or_else(|| current.label().to_owned());
                let (position, placement) =
                    position.unwrap_or((*current.position(), current.placement()));
                let annotation = Annotation::marker(id, &label, position, placement)?;
                if &annotation == current {
                    return Err(CommandError::NothingToChange);
                }
                (
                    tick,
                    Provenance::Operator,
                    EventPayload::AnnotationPut { annotation },
                )
            }
            Command::DeleteAnnotation { id, base_revision } => {
                check_base(&id, store.annotation_slot(&id), base_revision)?;
                (
                    tick,
                    Provenance::Operator,
                    EventPayload::AnnotationDeleted { id },
                )
            }
        };
        let event = CommittedEvent {
            epoch: self.store.epoch(),
            sequence,
            tick,
            provenance,
            payload,
        };
        let changes = self.store.apply(&event).map_err(CommandError::Rejected)?;
        self.journal.push(event);
        Ok(changes)
    }

    /// The state after the first `sequence` events of this journal,
    /// rebuilt by replaying them into an empty store. Never touches the
    /// live state.
    pub fn replay_to(&self, sequence: u64) -> Result<SceneStore, ApplyError> {
        let n = usize::try_from(sequence)
            .unwrap_or(usize::MAX)
            .min(self.journal.len());
        replay(
            self.store.site().clone(),
            self.store.epoch(),
            self.journal.get(..n).unwrap_or(&[]),
        )
    }
}

/// Replays committed events in order into an empty store for `site` and
/// `epoch`. Stops at the first event that does not apply.
pub fn replay(
    site: SiteFrame,
    epoch: u32,
    events: &[CommittedEvent],
) -> Result<SceneStore, ApplyError> {
    let mut store = SceneStore::new(site, epoch);
    for event in events {
        store.apply(event)?;
    }
    Ok(store)
}

/// Checks an edit's base revision with the kernel's precondition guard,
/// and says precisely why it failed.
fn check_base(id: &ObjectId, slot: Slot, base_revision: u64) -> Result<(), CommandError> {
    if base_revision_matches(slot, base_revision) {
        return Ok(());
    }
    Err(match slot {
        Slot::Vacant => CommandError::NotFound { id: id.clone() },
        Slot::Deleted { .. } => CommandError::Deleted { id: id.clone() },
        Slot::Live { revision } => CommandError::PreconditionFailed {
            id: id.clone(),
            base_revision,
            current: revision,
        },
    })
}

/// The first vacant `namespace:n` with `n > records`. IDs are never reused
/// within an epoch, because tombstoned IDs stay in the record count.
fn allocate(
    namespace: &str,
    records: usize,
    slot: impl Fn(&ObjectId) -> Slot,
) -> Result<ObjectId, CommandError> {
    let mut n = records;
    // At most `records` IDs can be taken, so this finds one within
    // `records + 1` tries.
    for _ in 0..=records {
        n = n.checked_add(1).ok_or(CommandError::IdSpaceExhausted)?;
        let id = ObjectId::from_parts("id", namespace, &n.to_string())?;
        if matches!(slot(&id), Slot::Vacant) {
            return Ok(id);
        }
    }
    Err(CommandError::IdSpaceExhausted)
}
