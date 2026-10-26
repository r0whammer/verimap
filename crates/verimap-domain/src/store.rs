//! The scene store: the reducer that applies committed events to scene
//! state. Every object write goes through the kernel's revision rules
//! (`slot_upsert`, `slot_delete`); `docs/verification.md` says which of
//! their properties are proved.

use std::collections::{BTreeMap, BTreeSet};

use verimap_kernel::{Decision, Slot, next_sequence, slot_delete, slot_upsert};

use crate::{
    Annotation, CommittedEvent, Entity, EventPayload, ObjectId, Provenance, ScenePoint, SiteFrame,
};

/// One object's state within an epoch: live with its value, or a tombstone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Record<T> {
    Live { revision: u64, value: T },
    Deleted { revision: u64 },
}

/// Every record of one object kind, in ID order.
pub(crate) type Records<T> = BTreeMap<ObjectId, Record<T>>;

impl<T> Record<T> {
    pub(crate) fn slot(record: Option<&Self>) -> Slot {
        match record {
            None => Slot::Vacant,
            Some(Record::Live { revision, .. }) => Slot::Live {
                revision: *revision,
            },
            Some(Record::Deleted { revision }) => Slot::Deleted {
                revision: *revision,
            },
        }
    }

    pub(crate) fn live(&self) -> Option<(u64, &T)> {
        match self {
            Record::Live { revision, value } => Some((*revision, value)),
            Record::Deleted { .. } => None,
        }
    }
}

/// Why a write the kernel decided against was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The same write was already applied.
    Duplicate,
    /// The write is older than the object's revision.
    Stale,
    /// Same revision, different content.
    Conflict,
    /// The object was deleted earlier in this epoch.
    Tombstoned,
    /// A delete of an object this epoch has never seen.
    Missing,
}

/// Why an event could not be applied. Nothing changes when `apply` fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyError {
    /// The event belongs to another scene epoch.
    EpochMismatch {
        /// This store's epoch.
        expected: u32,
        /// The event's epoch.
        found: u32,
    },
    /// The event's sequence was already applied.
    AlreadyApplied {
        /// The event's sequence.
        sequence: u64,
    },
    /// Events between the last applied and this one are missing.
    SequenceGap {
        /// The next sequence this store accepts.
        expected: u64,
        /// The event's sequence.
        found: u64,
    },
    /// The authority sequence is exhausted for this epoch.
    SequenceExhausted,
    /// The event's scene time differs from the time it must take effect:
    /// the current tick, or for a clock event its new tick.
    WrongTime {
        /// Required tick.
        expected: u64,
        /// The event's tick.
        found: u64,
    },
    /// A clock event that does not move the clock forward.
    ClockNotAdvanced {
        /// Current tick.
        tick: u64,
    },
    /// The provenance does not fit the payload.
    ProvenanceMismatch,
    /// An observation acquired after the current scene time.
    ObservedInFuture {
        /// Track ID.
        id: ObjectId,
    },
    /// An observation older than the track's latest; it must not
    /// overwrite newer state.
    LateObservation {
        /// Track ID.
        id: ObjectId,
    },
    /// An observation with a class different from the track's.
    ClassChanged {
        /// Track ID.
        id: ObjectId,
    },
    /// A position outside the site's scene bounds.
    OutsideSite {
        /// Object ID.
        id: ObjectId,
    },
    /// The kernel's revision rules refused the write.
    Refused {
        /// Object ID.
        id: ObjectId,
        /// Why.
        refusal: Refusal,
    },
}

impl core::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EpochMismatch { expected, found } => {
                write!(
                    f,
                    "event is for epoch {found}, scene is at epoch {expected}"
                )
            }
            Self::AlreadyApplied { sequence } => write!(f, "event {sequence} was already applied"),
            Self::SequenceGap { expected, found } => {
                write!(f, "expected event {expected}, got {found}")
            }
            Self::SequenceExhausted => write!(f, "the event sequence is exhausted"),
            Self::WrongTime { expected, found } => {
                write!(
                    f,
                    "event at tick {found} must take effect at tick {expected}"
                )
            }
            Self::ClockNotAdvanced { tick } => {
                write!(f, "clock event does not advance tick {tick}")
            }
            Self::ProvenanceMismatch => write!(f, "event provenance does not match its payload"),
            Self::ObservedInFuture { id } => {
                write!(f, "{id}: observation acquired after the current scene time")
            }
            Self::LateObservation { id } => {
                write!(f, "{id}: observation is older than the track's latest")
            }
            Self::ClassChanged { id } => {
                write!(f, "{id}: observation class differs from the track")
            }
            Self::OutsideSite { id } => write!(f, "{id}: position outside the site bounds"),
            Self::Refused { id, refusal } => write!(f, "{id}: write refused ({refusal:?})"),
        }
    }
}

impl std::error::Error for ApplyError {}

/// The objects an applied event touched. A touched ID that is no longer
/// live was deleted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    /// The scene clock moved.
    pub clock: bool,
    /// Entities created, updated, deleted, or whose freshness changed.
    pub entities: BTreeSet<ObjectId>,
    /// Annotations created, updated, or deleted.
    pub annotations: BTreeSet<ObjectId>,
}

impl Changes {
    /// Adds `other`'s changes to these.
    pub fn merge(&mut self, other: Changes) {
        self.clock |= other.clock;
        self.entities.extend(other.entities);
        self.annotations.extend(other.annotations);
    }

    /// Nothing changed.
    pub fn is_empty(&self) -> bool {
        !self.clock && self.entities.is_empty() && self.annotations.is_empty()
    }
}

/// Scene state for one site and epoch, built only by applying committed
/// events in sequence.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneStore {
    site: SiteFrame,
    epoch: u32,
    last_sequence: u64,
    tick: u64,
    entities: Records<Entity>,
    annotations: Records<Annotation>,
}

/// A validated change, ready to commit.
enum Plan {
    Clock {
        to_tick: u64,
        changed: BTreeSet<ObjectId>,
    },
    Entity {
        id: ObjectId,
        record: Record<Entity>,
    },
    Annotation {
        id: ObjectId,
        record: Record<Annotation>,
    },
}

impl SceneStore {
    /// An empty scene at tick 0, before any event.
    pub fn new(site: SiteFrame, epoch: u32) -> Self {
        Self {
            site,
            epoch,
            last_sequence: 0,
            tick: 0,
            entities: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }
    }

    /// The site this scene belongs to.
    pub fn site(&self) -> &SiteFrame {
        &self.site
    }

    /// Scene epoch.
    pub fn epoch(&self) -> u32 {
        self.epoch
    }

    /// Sequence of the last applied event; 0 before any.
    pub fn last_sequence(&self) -> u64 {
        self.last_sequence
    }

    /// Current scene time, in ticks.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Live entities, in ID order.
    pub fn entities(&self) -> impl Iterator<Item = (u64, &Entity)> {
        self.entities.values().filter_map(Record::live)
    }

    /// A live entity and its revision.
    pub fn entity(&self, id: &ObjectId) -> Option<(u64, &Entity)> {
        self.entities.get(id).and_then(Record::live)
    }

    /// The kernel slot of an entity ID.
    pub fn entity_slot(&self, id: &ObjectId) -> Slot {
        Record::slot(self.entities.get(id))
    }

    /// Live and deleted entity records; the next allocated track number
    /// derives from it.
    pub fn entity_record_count(&self) -> usize {
        self.entities.len()
    }

    /// Live annotations, in ID order.
    pub fn annotations(&self) -> impl Iterator<Item = (u64, &Annotation)> {
        self.annotations.values().filter_map(Record::live)
    }

    /// A live annotation and its revision.
    pub fn annotation(&self, id: &ObjectId) -> Option<(u64, &Annotation)> {
        self.annotations.get(id).and_then(Record::live)
    }

    /// The kernel slot of an annotation ID.
    pub fn annotation_slot(&self, id: &ObjectId) -> Slot {
        Record::slot(self.annotations.get(id))
    }

    /// Live and deleted annotation records.
    pub fn annotation_record_count(&self) -> usize {
        self.annotations.len()
    }

    pub(crate) fn records(&self) -> (&Records<Entity>, &Records<Annotation>) {
        (&self.entities, &self.annotations)
    }

    /// Applies one committed event, or changes nothing and says why.
    ///
    /// The event must be the next in sequence for this epoch. Every check
    /// runs before any mutation, so a failed apply leaves the store as it
    /// was.
    pub fn apply(&mut self, event: &CommittedEvent) -> Result<Changes, ApplyError> {
        let plan = self.plan(event)?;
        let mut changes = Changes::default();
        match plan {
            Plan::Clock { to_tick, changed } => {
                self.tick = to_tick;
                changes.clock = true;
                changes.entities = changed;
            }
            Plan::Entity { id, record } => {
                changes.entities.insert(id.clone());
                self.entities.insert(id, record);
            }
            Plan::Annotation { id, record } => {
                changes.annotations.insert(id.clone());
                self.annotations.insert(id, record);
            }
        }
        self.last_sequence = event.sequence;
        Ok(changes)
    }

    fn plan(&self, event: &CommittedEvent) -> Result<Plan, ApplyError> {
        if event.epoch != self.epoch {
            return Err(ApplyError::EpochMismatch {
                expected: self.epoch,
                found: event.epoch,
            });
        }
        let expected =
            next_sequence(self.last_sequence).map_err(|_| ApplyError::SequenceExhausted)?;
        if event.sequence <= self.last_sequence {
            return Err(ApplyError::AlreadyApplied {
                sequence: event.sequence,
            });
        }
        if event.sequence != expected {
            return Err(ApplyError::SequenceGap {
                expected,
                found: event.sequence,
            });
        }
        let seq = event.sequence;
        match (&event.payload, &event.provenance) {
            (EventPayload::ClockAdvanced { to_tick }, Provenance::Clock) => {
                if *to_tick <= self.tick {
                    return Err(ApplyError::ClockNotAdvanced { tick: self.tick });
                }
                self.check_time(event, *to_tick)?;
                let changed = self
                    .entities()
                    .filter(|(_, e)| e.freshness(self.tick) != e.freshness(*to_tick))
                    .map(|(_, e)| e.id().clone())
                    .collect();
                Ok(Plan::Clock {
                    to_tick: *to_tick,
                    changed,
                })
            }
            (EventPayload::EntityObserved { id, class, fix }, Provenance::Observation(source)) => {
                self.check_time(event, self.tick)?;
                if fix.acquired_tick > self.tick {
                    return Err(ApplyError::ObservedInFuture { id: id.clone() });
                }
                self.check_inside(id, &fix.position)?;
                let current = self.entities.get(id);
                let mut same_content = false;
                if let Some((_, existing)) = current.and_then(Record::live) {
                    if existing.class() != *class {
                        return Err(ApplyError::ClassChanged { id: id.clone() });
                    }
                    if fix.acquired_tick < existing.last_observed_tick() {
                        return Err(ApplyError::LateObservation { id: id.clone() });
                    }
                    same_content = existing.latest() == fix && existing.source() == source;
                }
                let t = slot_upsert(Record::slot(current), seq, same_content);
                let revision = applied(id, t.decision, t.slot)?;
                let value = match current.and_then(Record::live) {
                    Some((_, existing)) => {
                        let mut e = existing.clone();
                        e.observe(*fix, source.clone(), revision);
                        e
                    }
                    None => Entity::new(id.clone(), *class, *fix, source.clone(), revision),
                };
                Ok(Plan::Entity {
                    id: id.clone(),
                    record: Record::Live { revision, value },
                })
            }
            (EventPayload::EntityRemoved { id }, Provenance::Operator) => {
                self.check_time(event, self.tick)?;
                let t = slot_delete(Record::slot(self.entities.get(id)), seq);
                let revision = deleted(id, t.decision, t.slot)?;
                Ok(Plan::Entity {
                    id: id.clone(),
                    record: Record::Deleted { revision },
                })
            }
            (EventPayload::AnnotationPut { annotation }, Provenance::Operator) => {
                self.check_time(event, self.tick)?;
                let id = annotation.id();
                self.check_inside(id, annotation.position())?;
                let current = self.annotations.get(id);
                let same_content = current
                    .and_then(Record::live)
                    .is_some_and(|(_, a)| a == annotation);
                let t = slot_upsert(Record::slot(current), seq, same_content);
                let revision = applied(id, t.decision, t.slot)?;
                Ok(Plan::Annotation {
                    id: id.clone(),
                    record: Record::Live {
                        revision,
                        value: annotation.clone(),
                    },
                })
            }
            (EventPayload::AnnotationDeleted { id }, Provenance::Operator) => {
                self.check_time(event, self.tick)?;
                let t = slot_delete(Record::slot(self.annotations.get(id)), seq);
                let revision = deleted(id, t.decision, t.slot)?;
                Ok(Plan::Annotation {
                    id: id.clone(),
                    record: Record::Deleted { revision },
                })
            }
            _ => Err(ApplyError::ProvenanceMismatch),
        }
    }

    fn check_time(&self, event: &CommittedEvent, expected: u64) -> Result<(), ApplyError> {
        if event.tick != expected {
            return Err(ApplyError::WrongTime {
                expected,
                found: event.tick,
            });
        }
        Ok(())
    }

    fn check_inside(&self, id: &ObjectId, p: &ScenePoint) -> Result<(), ApplyError> {
        if !self.site.bounds().contains(p) {
            return Err(ApplyError::OutsideSite { id: id.clone() });
        }
        Ok(())
    }
}

/// The revision of an applied upsert, or why it was refused.
fn applied(id: &ObjectId, decision: Decision, slot: Slot) -> Result<u64, ApplyError> {
    match (decision, slot) {
        (Decision::Apply, Slot::Live { revision }) => Ok(revision),
        (decision, _) => Err(refused(id, decision)),
    }
}

/// The revision of an applied delete, or why it was refused.
fn deleted(id: &ObjectId, decision: Decision, slot: Slot) -> Result<u64, ApplyError> {
    match (decision, slot) {
        (Decision::Apply, Slot::Deleted { revision }) => Ok(revision),
        (decision, _) => Err(refused(id, decision)),
    }
}

fn refused(id: &ObjectId, decision: Decision) -> ApplyError {
    let refusal = match decision {
        Decision::Duplicate => Refusal::Duplicate,
        Decision::Stale => Refusal::Stale,
        Decision::Conflict => Refusal::Conflict,
        Decision::Tombstoned => Refusal::Tombstoned,
        Decision::Missing => Refusal::Missing,
        // The kernel's proved contract (docs/verification.md rows 2 and 4)
        // pairs Apply with the resulting slot, so this arm is unreachable
        // through `applied`/`deleted`. Report it as a conflict rather than
        // panicking.
        Decision::Apply => Refusal::Conflict,
    };
    ApplyError::Refused {
        id: id.clone(),
        refusal,
    }
}
