//! VeriMap's domain: what scene state means and which values are valid.
//!
//! Everything that enters scene state is validated here first, using the
//! extracted kernel's guards where one exists (`verimap-kernel`), and a
//! rejected value never changes state.
//!
//! Three separate concepts:
//!
//! 1. an [`ObservationEnvelope`] is what a source reported; the
//!    [`Normalizer`] validates it into a [`Detection`];
//! 2. a [`Command`] is a requested change, such as an estimator's track
//!    update ([`associate`]) or an operator edit;
//! 3. a [`CommittedEvent`] is a validated, sequenced transition that the
//!    [`Authority`] journals and the [`SceneStore`] applies. Replaying the
//!    journal ([`replay`]) rebuilds the same state, which
//!    [`SceneStore::state_hash`] makes cheap to compare.

#![forbid(unsafe_code)]

mod annotation;
mod authority;
mod canonical;
mod entity;
mod error;
mod event;
mod ids;
mod observation;
mod point;
mod site;
mod store;
mod text;
mod tracker;

pub use annotation::{Annotation, Placement};
pub use authority::{
    Authority, Command, CommandError, OPERATOR_NAMESPACE, TRACK_NAMESPACE, TrackTarget, replay,
};
pub use canonical::fnv1a64;
pub use entity::{
    Confidence, EXPIRE_AFTER_TICKS, Entity, EntityClass, Freshness, HistoryEntry, ObservationRef,
    STALE_AFTER_TICKS, TICK_MS, TrackFix, Uncertainty,
};
pub use error::DomainError;
pub use event::{CommittedEvent, EVENT_SCHEMA_VERSION, EventPayload, Provenance};
pub use ids::{ObjectId, SourceId};
pub use observation::{
    Detection, Normalizer, ObservationEnvelope, ObservationError, ObservationPayload,
    SCENE_FRAME_ID, SourceRegistration,
};
pub use point::{ScenePoint, metres_to_mm};
pub use site::{GeodeticAnchor, SceneBounds, SiteFrame, SiteId};
pub use store::{ApplyError, Changes, Refusal, SceneStore};
pub use text::{validate_label, validate_slug};
pub use tracker::{Association, TrackerConfig, associate};
/// Kernel types that appear in the domain's public API.
pub use verimap_kernel::{AxisBoundsMm, Slot};
