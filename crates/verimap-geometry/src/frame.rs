//! Named, versioned frames. A lookup that misses returns
//! [`GeometryError::UnknownFrame`] and no point.

use std::collections::BTreeMap;

use crate::{GeometryError, RigidTransform};

/// A frame name: 1 to [`FrameId::MAX_BYTES`] bytes, with no ASCII control
/// characters. Names are case-sensitive and are not slugs; a sensor frame
/// may carry a vendor token.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FrameId(String);

impl FrameId {
    /// Longest accepted name, in bytes.
    pub const MAX_BYTES: usize = 64;

    /// Validates `name`.
    pub fn new(name: &str) -> Result<Self, GeometryError> {
        if name.is_empty() {
            return Err(GeometryError::InvalidFrameName {
                reason: "must not be empty",
            });
        }
        if name.len() > Self::MAX_BYTES {
            return Err(GeometryError::InvalidFrameName { reason: "too long" });
        }
        if name.chars().any(char::is_control) {
            return Err(GeometryError::InvalidFrameName {
                reason: "must not contain control characters",
            });
        }
        Ok(Self(name.to_owned()))
    }

    /// The name text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for FrameId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One registered `T_parent_from_child` at a calibration version.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameSample {
    /// Frame this transform is expressed in.
    pub parent: FrameId,
    /// Frame being located.
    pub child: FrameId,
    /// Calibration version. A different version is a different transform.
    pub version: u32,
    /// `T_parent_from_child`.
    pub transform: RigidTransform,
}

/// The frames a session knows. Keys are `(child, version)`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FrameRegistry {
    by_child: BTreeMap<(FrameId, u32), FrameSample>,
}

impl FrameRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self {
            by_child: BTreeMap::new(),
        }
    }

    /// Registers `sample`. The same child, version, and transform is a no-op.
    /// A different transform at that key is [`GeometryError::FrameConflict`]
    /// and the registry is unchanged.
    pub fn insert(&mut self, sample: FrameSample) -> Result<(), GeometryError> {
        let key = (sample.child.clone(), sample.version);
        if let Some(existing) = self.by_child.get(&key) {
            if existing == &sample {
                return Ok(());
            }
            return Err(GeometryError::FrameConflict {
                frame: sample.child.as_str().to_owned(),
                version: sample.version,
            });
        }
        self.by_child.insert(key, sample);
        Ok(())
    }

    /// `T_parent_from_child` for `child` at `version`, or
    /// [`GeometryError::UnknownFrame`] when that pair was never registered.
    pub fn parent_from_child(
        &self,
        child: &FrameId,
        version: u32,
    ) -> Result<&RigidTransform, GeometryError> {
        self.by_child
            .get(&(child.clone(), version))
            .map(|s| &s.transform)
            .ok_or_else(|| GeometryError::UnknownFrame {
                frame: child.as_str().to_owned(),
                version,
            })
    }

    /// How many transforms are registered.
    pub fn len(&self) -> usize {
        self.by_child.len()
    }

    /// No transforms are registered.
    pub fn is_empty(&self) -> bool {
        self.by_child.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Quaternion;

    fn sample(child: &str, version: u32, east_m: f64) -> FrameSample {
        FrameSample {
            parent: FrameId::new("scene").unwrap(),
            child: FrameId::new(child).unwrap(),
            version,
            transform: RigidTransform::from_parts(Quaternion::identity(), [east_m, 0.0, 0.0])
                .unwrap(),
        }
    }

    #[test]
    fn names_are_bounded_text() {
        let id = FrameId::new("lidar-1").unwrap();
        assert_eq!(id.as_str(), "lidar-1");
        assert_eq!(id.to_string(), "lidar-1");
        let empty = FrameId::new("").unwrap_err();
        assert!(empty.to_string().contains("empty"));
        assert!(matches!(
            empty,
            GeometryError::InvalidFrameName {
                reason: "must not be empty"
            }
        ));
        assert!(matches!(
            FrameId::new("line\n"),
            Err(GeometryError::InvalidFrameName { .. })
        ));
        let long = "a".repeat(FrameId::MAX_BYTES + 1);
        assert!(matches!(
            FrameId::new(&long),
            Err(GeometryError::InvalidFrameName { reason: "too long" })
        ));
        assert!(FrameId::new(&"a".repeat(FrameId::MAX_BYTES)).is_ok());
    }

    #[test]
    fn unknown_version_and_conflicts_do_not_change_the_registry() {
        let mut reg = FrameRegistry::new();
        assert!(reg.is_empty());
        let child = FrameId::new("lidar").unwrap();
        assert_eq!(
            reg.parent_from_child(&child, 1).map(|t| t.translation_m()),
            Err(GeometryError::UnknownFrame {
                frame: "lidar".into(),
                version: 1
            })
        );
        reg.insert(sample("lidar", 1, 0.2)).unwrap();
        assert_eq!(reg.len(), 1);
        assert!(!reg.is_empty());
        // Repeating the same registration is a no-op.
        reg.insert(sample("lidar", 1, 0.2)).unwrap();
        assert_eq!(reg.len(), 1);
        let before = reg.clone();
        let conflict = reg.insert(sample("lidar", 1, 0.4)).unwrap_err();
        assert!(conflict.to_string().contains("already registered"));
        assert_eq!(
            conflict,
            GeometryError::FrameConflict {
                frame: "lidar".into(),
                version: 1
            }
        );
        assert_eq!(reg, before);
        // A new version is a different transform and does not replace version 1.
        reg.insert(sample("lidar", 2, 0.4)).unwrap();
        assert_eq!(
            reg.parent_from_child(&child, 1).unwrap().translation_m()[0],
            0.2
        );
        assert_eq!(
            reg.parent_from_child(&child, 2).unwrap().translation_m()[0],
            0.4
        );
        let err = reg.parent_from_child(&child, 3).unwrap_err();
        assert!(err.to_string().contains("version 3"));
        assert!(GeometryError::RotationNotUnit.to_string().contains("norm"));
        assert!(
            GeometryError::NonFinite { field: "quat.x" }
                .to_string()
                .contains("quat.x")
        );
    }
}
