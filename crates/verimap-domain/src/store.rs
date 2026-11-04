//! The scene store: the reducer that applies committed events to scene
//! state. Every object write goes through the kernel's revision rules
//! (`slot_upsert`, `slot_delete`); `docs/verification.md` says which of
//! their properties are proved.

use std::collections::{BTreeMap, BTreeSet};

use verimap_kernel::{Decision, Slot, next_sequence, slot_delete, slot_upsert};

use crate::{
    Annotation, ChunkId, CommittedEvent, Entity, EventPayload, ObjectId, Provenance, ScenePoint,
    SiteFrame, SurfaceChunk,
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
    /// The kernel's revision rules refused a chunk write.
    ChunkRefused {
        /// Chunk id.
        id: ChunkId,
        /// Why.
        refusal: Refusal,
    },
    /// The chunk footprint misses the site.
    ChunkOutsideSite {
        /// Chunk id.
        id: ChunkId,
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
            Self::ChunkRefused { id, refusal } => {
                write!(
                    f,
                    "chunk {},{}: write refused ({refusal:?})",
                    id.ix(),
                    id.iy()
                )
            }
            Self::ChunkOutsideSite { id } => {
                write!(f, "chunk {},{}: outside the site", id.ix(), id.iy())
            }
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
    /// Chunks created, updated, or deleted.
    pub chunks: BTreeSet<ChunkId>,
}

impl Changes {
    /// Adds `other`'s changes to these.
    pub fn merge(&mut self, other: Changes) {
        self.clock |= other.clock;
        self.entities.extend(other.entities);
        self.annotations.extend(other.annotations);
        self.chunks.extend(other.chunks);
    }

    /// Nothing changed.
    pub fn is_empty(&self) -> bool {
        !self.clock
            && self.entities.is_empty()
            && self.annotations.is_empty()
            && self.chunks.is_empty()
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
    chunks: BTreeMap<ChunkId, Record<SurfaceChunk>>,
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
    Chunk {
        id: ChunkId,
        record: Record<SurfaceChunk>,
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
            chunks: BTreeMap::new(),
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

    /// Live chunks, in id order.
    pub fn chunks(&self) -> impl Iterator<Item = (u64, &SurfaceChunk)> {
        self.chunks.values().filter_map(Record::live)
    }

    /// A live chunk and its revision.
    pub fn chunk(&self, id: &ChunkId) -> Option<(u64, &SurfaceChunk)> {
        self.chunks.get(id).and_then(Record::live)
    }

    /// The kernel slot of a chunk id.
    pub fn chunk_slot(&self, id: &ChunkId) -> Slot {
        Record::slot(self.chunks.get(id))
    }

    pub(crate) fn chunk_records(&self) -> &BTreeMap<ChunkId, Record<SurfaceChunk>> {
        &self.chunks
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
            Plan::Chunk { id, record } => {
                changes.chunks.insert(id);
                self.chunks.insert(id, record);
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
            (
                EventPayload::ChunkReplaced { chunk },
                Provenance::Operator | Provenance::Observation(_),
            ) => {
                self.check_time(event, self.tick)?;
                if !chunk.id().intersects_xy(self.site.bounds()) {
                    return Err(ApplyError::ChunkOutsideSite { id: chunk.id() });
                }
                let id = chunk.id();
                let current = self.chunks.get(&id);
                let same_content = current
                    .and_then(Record::live)
                    .is_some_and(|(_, stored)| stored == chunk);
                let t = slot_upsert(Record::slot(current), seq, same_content);
                let revision = applied_chunk(&id, t.decision, t.slot)?;
                Ok(Plan::Chunk {
                    id,
                    record: Record::Live {
                        revision,
                        value: chunk.clone(),
                    },
                })
            }
            (EventPayload::ChunkDeleted { id }, Provenance::Operator) => {
                self.check_time(event, self.tick)?;
                let t = slot_delete(Record::slot(self.chunks.get(id)), seq);
                let revision = deleted_chunk(id, t.decision, t.slot)?;
                Ok(Plan::Chunk {
                    id: *id,
                    record: Record::Deleted { revision },
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

fn applied_chunk(id: &ChunkId, decision: Decision, slot: Slot) -> Result<u64, ApplyError> {
    match (decision, slot) {
        (Decision::Apply, Slot::Live { revision }) => Ok(revision),
        (decision, _) => Err(chunk_refused(id, decision)),
    }
}

fn deleted_chunk(id: &ChunkId, decision: Decision, slot: Slot) -> Result<u64, ApplyError> {
    match (decision, slot) {
        (Decision::Apply, Slot::Deleted { revision }) => Ok(revision),
        (decision, _) => Err(chunk_refused(id, decision)),
    }
}

fn chunk_refused(id: &ChunkId, decision: Decision) -> ApplyError {
    let refusal = match decision {
        Decision::Duplicate => Refusal::Duplicate,
        Decision::Stale => Refusal::Stale,
        Decision::Conflict => Refusal::Conflict,
        Decision::Tombstoned => Refusal::Tombstoned,
        Decision::Missing => Refusal::Missing,
        Decision::Apply => Refusal::Conflict,
    };
    ApplyError::ChunkRefused { id: *id, refusal }
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

#[cfg(test)]
pub(crate) mod tests {
    /// Pose `store` at the last representable sequence so the next commit
    /// reports exhaustion. Only tests should do this.
    pub(crate) fn exhaust_sequence(store: &mut SceneStore) {
        store.last_sequence = u64::MAX;
    }

    use super::*;
    use crate::{
        Annotation, CELL_COUNT, Cell, ChunkId, Confidence, Entity, EntityClass, EventPayload,
        GeodeticAnchor, ObjectId, ObservationRef, Placement, Provenance, SCENE_FRAME, SceneBounds,
        ScenePoint, SiteFrame, SiteId, SourceId, SurfaceChunk, TrackFix, Uncertainty,
    };
    use verimap_kernel::Decision;

    fn site() -> SiteFrame {
        SiteFrame::new(
            SiteId::new("s").unwrap(),
            "Site",
            GeodeticAnchor::new(1.0, 2.0, 0.0).unwrap(),
            SceneBounds::new((-100_000, 100_000), (-100_000, 100_000), (-20_000, 80_000)).unwrap(),
        )
        .unwrap()
    }

    fn source(sequence: u64) -> ObservationRef {
        ObservationRef {
            source: SourceId::new("s", "cam").unwrap(),
            sequence,
        }
    }

    fn fix(tick: u64, x_mm: i64) -> TrackFix {
        TrackFix {
            acquired_tick: tick,
            position: ScenePoint::new(x_mm, 0, 0).unwrap(),
            uncertainty: Uncertainty::new(1_000).unwrap(),
            confidence: Confidence::new(5_000).unwrap(),
        }
    }

    fn id(s: &str) -> ObjectId {
        ObjectId::parse("id", s).unwrap()
    }

    fn observe(
        seq: u64,
        tick: u64,
        track: &str,
        at: TrackFix,
        src: ObservationRef,
    ) -> CommittedEvent {
        CommittedEvent {
            epoch: 1,
            sequence: seq,
            tick,
            provenance: Provenance::Observation(src),
            payload: EventPayload::EntityObserved {
                id: id(track),
                class: EntityClass::Vehicle,
                fix: at,
            },
        }
    }

    #[test]
    fn wrong_time_and_exhausted_sequence_change_nothing() {
        let mut store = SceneStore::new(site(), 1);
        let before = store.clone();
        let clock = CommittedEvent {
            epoch: 1,
            sequence: 1,
            tick: 0,
            provenance: Provenance::Clock,
            payload: EventPayload::ClockAdvanced { to_tick: 5 },
        };
        assert_eq!(
            store.apply(&clock),
            Err(ApplyError::WrongTime {
                expected: 5,
                found: 0
            })
        );
        let early = observe(1, 3, "trk:1", fix(0, 0), source(1));
        assert_eq!(
            store.apply(&early),
            Err(ApplyError::WrongTime {
                expected: 0,
                found: 3
            })
        );
        store.last_sequence = u64::MAX;
        assert_eq!(store.apply(&clock), Err(ApplyError::SequenceExhausted));
        store.last_sequence = before.last_sequence;
        assert_eq!(store, before);
    }

    #[test]
    fn same_revision_and_missing_deletes_are_refused() {
        let mut store = SceneStore::new(site(), 1);
        let src = source(1);
        let at = fix(0, 0);
        let entity = Entity::new(id("trk:1"), EntityClass::Vehicle, at, src.clone(), 1);
        store.entities.insert(
            id("trk:1"),
            Record::Live {
                revision: 1,
                value: entity,
            },
        );
        let duplicate = observe(1, 0, "trk:1", at, src.clone());
        assert_eq!(
            store.apply(&duplicate),
            Err(ApplyError::Refused {
                id: id("trk:1"),
                refusal: Refusal::Duplicate
            })
        );
        let conflict = observe(1, 0, "trk:1", fix(0, 500), src);
        assert_eq!(
            store.apply(&conflict),
            Err(ApplyError::Refused {
                id: id("trk:1"),
                refusal: Refusal::Conflict
            })
        );

        store.entities.insert(
            id("trk:2"),
            Record::Live {
                revision: 9,
                value: Entity::new(id("trk:2"), EntityClass::Vehicle, fix(0, 0), source(2), 9),
            },
        );
        let stale = observe(1, 0, "trk:2", fix(0, 0), source(2));
        assert_eq!(
            store.apply(&stale),
            Err(ApplyError::Refused {
                id: id("trk:2"),
                refusal: Refusal::Stale
            })
        );

        let missing = CommittedEvent {
            epoch: 1,
            sequence: 1,
            tick: 0,
            provenance: Provenance::Operator,
            payload: EventPayload::EntityRemoved { id: id("trk:9") },
        };
        assert_eq!(
            store.apply(&missing),
            Err(ApplyError::Refused {
                id: id("trk:9"),
                refusal: Refusal::Missing
            })
        );
        let marker = Annotation::marker(
            id("op:1"),
            "Gate",
            ScenePoint::new(0, 0, 4_000).unwrap(),
            Placement::GroundPlane,
        )
        .unwrap();
        let gone = CommittedEvent {
            epoch: 1,
            sequence: 1,
            tick: 0,
            provenance: Provenance::Operator,
            payload: EventPayload::AnnotationDeleted { id: id("op:9") },
        };
        assert_eq!(
            store.apply(&gone),
            Err(ApplyError::Refused {
                id: id("op:9"),
                refusal: Refusal::Missing
            })
        );
        store.annotations.insert(
            id("op:1"),
            Record::Live {
                revision: 1,
                value: marker.clone(),
            },
        );
        let again = CommittedEvent {
            epoch: 1,
            sequence: 1,
            tick: 0,
            provenance: Provenance::Operator,
            payload: EventPayload::AnnotationPut { annotation: marker },
        };
        assert_eq!(
            store.apply(&again),
            Err(ApplyError::Refused {
                id: id("op:1"),
                refusal: Refusal::Duplicate
            })
        );
        assert_eq!(store.last_sequence(), 0);
        // The kernel pairs Apply with a new slot. This arm is the fallback
        // if that pairing is ever broken.
        assert_eq!(
            refused(&id("trk:1"), Decision::Apply),
            ApplyError::Refused {
                id: id("trk:1"),
                refusal: Refusal::Conflict
            }
        );
    }

    #[test]
    fn changes_merge_and_errors_explain_themselves() {
        let mut c = Changes::default();
        assert!(c.is_empty());
        c.merge(Changes {
            clock: true,
            entities: BTreeSet::from([id("trk:1")]),
            annotations: BTreeSet::from([id("op:1")]),
            chunks: BTreeSet::new(),
        });
        assert!(!c.is_empty());
        let samples = [
            ApplyError::EpochMismatch {
                expected: 1,
                found: 2,
            },
            ApplyError::AlreadyApplied { sequence: 4 },
            ApplyError::SequenceGap {
                expected: 5,
                found: 8,
            },
            ApplyError::SequenceExhausted,
            ApplyError::WrongTime {
                expected: 1,
                found: 0,
            },
            ApplyError::ClockNotAdvanced { tick: 3 },
            ApplyError::ProvenanceMismatch,
            ApplyError::ObservedInFuture { id: id("trk:1") },
            ApplyError::LateObservation { id: id("trk:1") },
            ApplyError::ClassChanged { id: id("trk:1") },
            ApplyError::OutsideSite { id: id("trk:1") },
            ApplyError::Refused {
                id: id("trk:1"),
                refusal: Refusal::Tombstoned,
            },
            ApplyError::ChunkRefused {
                id: crate::ChunkId::new(1, -1).unwrap(),
                refusal: Refusal::Stale,
            },
            ApplyError::ChunkOutsideSite {
                id: crate::ChunkId::new(1, -1).unwrap(),
            },
        ];
        for e in samples {
            assert!(!e.to_string().is_empty(), "{e:?}");
        }
    }

    fn blank_chunk(ix: i32, iy: i32) -> SurfaceChunk {
        SurfaceChunk::from_cells(
            ChunkId::new(ix, iy).unwrap(),
            SCENE_FRAME,
            &vec![Cell::unknown(); CELL_COUNT],
        )
        .unwrap()
    }

    #[test]
    fn chunk_writes_follow_the_same_revision_rules() {
        let mut store = SceneStore::new(site(), 1);
        let id = ChunkId::new(0, 0).unwrap();
        let chunk = blank_chunk(0, 0);
        store.chunks.insert(
            id,
            Record::Live {
                revision: 1,
                value: chunk.clone(),
            },
        );
        let duplicate = CommittedEvent {
            epoch: 1,
            sequence: 1,
            tick: 0,
            provenance: Provenance::Operator,
            payload: EventPayload::ChunkReplaced {
                chunk: chunk.clone(),
            },
        };
        assert_eq!(
            store.apply(&duplicate),
            Err(ApplyError::ChunkRefused {
                id,
                refusal: Refusal::Duplicate,
            })
        );
        let mut cells = vec![Cell::unknown(); CELL_COUNT];
        cells[0] = Cell::observed(10, 1).unwrap();
        let other = SurfaceChunk::from_cells(id, SCENE_FRAME, &cells).unwrap();
        let conflict = CommittedEvent {
            payload: EventPayload::ChunkReplaced { chunk: other },
            ..duplicate.clone()
        };
        assert_eq!(
            store.apply(&conflict),
            Err(ApplyError::ChunkRefused {
                id,
                refusal: Refusal::Conflict,
            })
        );
        store.chunks.insert(
            ChunkId::new(1, 0).unwrap(),
            Record::Live {
                revision: 9,
                value: blank_chunk(1, 0),
            },
        );
        let stale = CommittedEvent {
            payload: EventPayload::ChunkReplaced {
                chunk: blank_chunk(1, 0),
            },
            ..duplicate.clone()
        };
        assert_eq!(
            store.apply(&stale),
            Err(ApplyError::ChunkRefused {
                id: ChunkId::new(1, 0).unwrap(),
                refusal: Refusal::Stale,
            })
        );
        let missing = CommittedEvent {
            payload: EventPayload::ChunkDeleted {
                id: ChunkId::new(-1, 0).unwrap(),
            },
            ..duplicate.clone()
        };
        assert_eq!(
            store.apply(&missing),
            Err(ApplyError::ChunkRefused {
                id: ChunkId::new(-1, 0).unwrap(),
                refusal: Refusal::Missing,
            })
        );
        let outside = CommittedEvent {
            payload: EventPayload::ChunkReplaced {
                chunk: blank_chunk(20, 0),
            },
            ..duplicate
        };
        assert_eq!(
            store.apply(&outside),
            Err(ApplyError::ChunkOutsideSite {
                id: ChunkId::new(20, 0).unwrap(),
            })
        );
        assert_eq!(
            chunk_refused(&id, Decision::Apply),
            ApplyError::ChunkRefused {
                id,
                refusal: Refusal::Conflict,
            }
        );
        assert_eq!(store.last_sequence(), 0);
    }
}
