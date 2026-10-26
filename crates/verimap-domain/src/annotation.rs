//! Operator annotations: explicit, operator-made facts kept separate from
//! sensor-derived state.

use crate::{DomainError, ObjectId, ScenePoint, validate_label};

/// Where a placed position's height came from. Every annotation position
/// records it, so a height is never presented as measured when it was
/// assumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Placement {
    /// Placed on the scene frame's ground plane, z = 0 m (the synthetic
    /// ground of the demo site). The height is an assumption, not a pick
    /// of reconstructed surface.
    GroundPlane,
}

impl Placement {
    /// Stable lowercase name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GroundPlane => "ground_plane",
        }
    }
}

/// An operator-placed marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    id: ObjectId,
    label: String,
    position: ScenePoint,
    placement: Placement,
}

impl Annotation {
    /// Longest accepted label, in bytes.
    pub const LABEL_MAX_BYTES: usize = 128;

    /// Validates the label. A ground-plane placement forces `z = 0`.
    pub fn marker(
        id: ObjectId,
        label: &str,
        position: ScenePoint,
        placement: Placement,
    ) -> Result<Self, DomainError> {
        validate_label("label", label, Self::LABEL_MAX_BYTES)?;
        let position = match placement {
            Placement::GroundPlane => position.with_z(0)?,
        };
        Ok(Self {
            id,
            label: label.to_owned(),
            position,
            placement,
        })
    }

    /// Stable annotation ID.
    pub fn id(&self) -> &ObjectId {
        &self.id
    }

    /// Display label. Untrusted text: render as text only.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Marker position.
    pub fn position(&self) -> &ScenePoint {
        &self.position
    }

    /// Where the position's height came from.
    pub fn placement(&self) -> Placement {
        self.placement
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ground_plane_markers_sit_at_zero() {
        let id = ObjectId::parse("id", "op:1").unwrap();
        let p = ScenePoint::new(1_000, 2_000, 3_000).unwrap();
        let m = Annotation::marker(id, "Gate", p, Placement::GroundPlane).unwrap();
        assert_eq!(m.position().z_mm(), 0);
        assert_eq!(m.position().x_mm(), 1_000);
    }

    #[test]
    fn labels_are_validated() {
        let id = || ObjectId::parse("id", "op:1").unwrap();
        let p = ScenePoint::new(0, 0, 0).unwrap();
        assert!(Annotation::marker(id(), "", p, Placement::GroundPlane).is_err());
        assert!(Annotation::marker(id(), "a\u{0}b", p, Placement::GroundPlane).is_err());
        let long = "x".repeat(Annotation::LABEL_MAX_BYTES + 1);
        assert!(Annotation::marker(id(), &long, p, Placement::GroundPlane).is_err());
        // Markup is just text.
        assert!(Annotation::marker(id(), "<b>x</b>", p, Placement::GroundPlane).is_ok());
    }
}
