//! Identifiers. An object ID is independent of the object's coordinates:
//! moving an entity never changes its identity.

use core::fmt;

use crate::{DomainError, validate_slug};

/// Identifier of a scene object (an entity track or an annotation), written
/// `namespace:local` with both parts slugs.
///
/// The namespace says who allocated the ID (`trk` for the baseline tracker,
/// `op` for operator annotations, or a source's own namespace), so a
/// source-provided track ID can never collide with another source's. A track
/// ID is not a verified real-world identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(String);

impl ObjectId {
    /// Longest accepted namespace, in bytes.
    pub const NAMESPACE_MAX_BYTES: usize = 32;
    /// Longest accepted local part, in bytes.
    pub const LOCAL_MAX_BYTES: usize = 63;

    /// Parses `namespace:local`.
    pub fn parse(field: &'static str, s: &str) -> Result<Self, DomainError> {
        let Some((namespace, local)) = s.split_once(':') else {
            return Err(DomainError::InvalidText {
                field,
                reason: "must be namespace:local",
            });
        };
        Self::from_parts(field, namespace, local)
    }

    /// Joins a namespace and a local part, validating both.
    pub fn from_parts(
        field: &'static str,
        namespace: &str,
        local: &str,
    ) -> Result<Self, DomainError> {
        validate_slug(field, namespace, Self::NAMESPACE_MAX_BYTES)?;
        validate_slug(field, local, Self::LOCAL_MAX_BYTES)?;
        Ok(Self(format!("{namespace}:{local}")))
    }

    /// The full `namespace:local` text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Identifier of an observation source (a sensor or adapter), a slug.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceId(String);

impl SourceId {
    /// Longest accepted identifier, in bytes.
    pub const MAX_BYTES: usize = 64;

    /// Validates a slug identifier.
    pub fn new(field: &'static str, s: &str) -> Result<Self, DomainError> {
        validate_slug(field, s, Self::MAX_BYTES)?;
        Ok(Self(s.to_owned()))
    }

    /// The identifier text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_ids_are_namespaced_slugs() {
        let id = ObjectId::parse("id", "trk:12").unwrap();
        assert_eq!(id.as_str(), "trk:12");
        assert_eq!(
            ObjectId::from_parts("id", "op", "7").unwrap(),
            ObjectId::parse("id", "op:7").unwrap()
        );
        for bad in [
            "", "trk", ":1", "trk:", "trk:1:2", "TRK:1", "trk: 1", "trk:-1",
        ] {
            assert!(ObjectId::parse("id", bad).is_err(), "{bad:?} accepted");
        }
        let long_local = format!("a:{}", "x".repeat(ObjectId::LOCAL_MAX_BYTES + 1));
        assert!(ObjectId::parse("id", &long_local).is_err());
    }

    #[test]
    fn ids_order_by_text() {
        let a = ObjectId::parse("id", "trk:10").unwrap();
        let b = ObjectId::parse("id", "trk:9").unwrap();
        // Lexicographic, not numeric: canonical order is the byte order.
        assert!(a < b);
    }
}
