//! Frames and rigid transforms for sensor geometry.
//!
//! This crate is outside the Charon/Aeneas proof boundary: every value here
//! is `f64`. Directions are [`RigidTransform`] values named
//! `T_parent_from_child`. A missing frame or version is a [`GeometryError`]
//! and produces no point, so a caller can refuse the observation before it
//! touches scene state.
//!
//! Conventions are documented in `docs/coordinates.md`.

#![forbid(unsafe_code)]

mod error;
mod frame;
mod quat;
mod transform;

pub use error::GeometryError;
pub use frame::{FrameId, FrameRegistry, FrameSample};
pub use quat::{Quaternion, ROTATION_NORM_TOLERANCE};
pub use transform::RigidTransform;
