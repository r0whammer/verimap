//! VeriMap's wire format: versioned DTOs, bounded parsing, JSON Schema
//! generation, and conversion between DTOs and validated domain values.
//!
//! Serialization lives here and never in `verimap-kernel` or
//! `verimap-domain`. TypeScript types in `web/src/generated` are generated
//! from the schemas this crate emits (`just gen`); they describe data but do
//! not validate it, so every input is validated here, in Rust. See
//! docs/protocol.md for the compatibility rules.

#![forbid(unsafe_code)]

pub mod bridge;
pub mod scene;
pub mod site;
mod u64s;
mod version;

pub use u64s::U64String;
pub use version::{PROTOCOL_VERSION, ProtocolVersion};
