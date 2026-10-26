//! Committed events: validated, sequenced state transitions. The journal of
//! committed events is the replay source of truth.

use crate::{Annotation, EntityClass, ObjectId, ObservationRef, TrackFix};

/// Version of the committed-event format and reducer rules. A recorded
/// journal from another version is rejected or migrated explicitly.
pub const EVENT_SCHEMA_VERSION: u16 = 1;

/// Who or what caused an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provenance {
    /// The session's scene clock.
    Clock,
    /// An accepted sensor observation, through the estimator.
    Observation(ObservationRef),
    /// An explicit operator command.
    Operator,
}

/// What an event changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventPayload {
    /// The scene clock moves forward to `to_tick`.
    ClockAdvanced {
        /// New scene time, in ticks.
        to_tick: u64,
    },
    /// A track was created or updated from an observation.
    EntityObserved {
        /// Track ID.
        id: ObjectId,
        /// Believed class; fixed for the life of the track.
        class: EntityClass,
        /// The accepted fix.
        fix: TrackFix,
    },
    /// A track was deleted explicitly. Its ID stays tombstoned.
    EntityRemoved {
        /// Track ID.
        id: ObjectId,
    },
    /// An annotation was created or replaced.
    AnnotationPut {
        /// The full new annotation.
        annotation: Annotation,
    },
    /// An annotation was deleted. Its ID stays tombstoned.
    AnnotationDeleted {
        /// Annotation ID.
        id: ObjectId,
    },
}

/// A validated, sequenced state transition.
///
/// `(scene, epoch, sequence)` identifies an event uniquely. Sequences
/// increase strictly by one within an epoch and never wrap. An object's
/// revision is the sequence of the event that last changed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedEvent {
    /// Scene epoch the event belongs to.
    pub epoch: u32,
    /// Authority log sequence, from 1.
    pub sequence: u64,
    /// Logical scene time at which the event took effect, in ticks.
    pub tick: u64,
    /// Cause of the event.
    pub provenance: Provenance,
    /// The change.
    pub payload: EventPayload,
}
