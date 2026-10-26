//! VeriMap's domain: what scene state means and which values are valid.
//!
//! Everything that enters scene state is validated here first, using the
//! extracted kernel's guards where one exists (`verimap-kernel`), and a
//! rejected value never changes state. The scene store, commands, and
//! committed events arrive with milestone M1; this crate currently holds the
//! validated site frame.

#![forbid(unsafe_code)]

mod error;
mod site;
mod text;

pub use error::DomainError;
pub use site::{GeodeticAnchor, SceneBounds, SiteFrame, SiteId};
pub use text::{validate_label, validate_slug};
/// Kernel types that appear in the domain's public API.
pub use verimap_kernel::{AxisBoundsMm, Confidence};
