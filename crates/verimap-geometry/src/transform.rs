//! Rigid `T_parent_from_child`: rotate a child-frame point, then add the
//! child origin expressed in the parent. Lengths are metres.

use crate::{GeometryError, Quaternion};

/// `T_parent_from_child`. Both the rotation and the translation are finite;
/// the rotation is a unit quaternion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidTransform {
    rotation: Quaternion,
    /// Child origin in the parent frame, metres.
    translation_m: [f64; 3],
}

impl RigidTransform {
    /// Identity: no rotation, child origin at the parent origin.
    pub fn identity() -> Self {
        Self {
            rotation: Quaternion::identity(),
            translation_m: [0.0, 0.0, 0.0],
        }
    }

    /// `rotation` is `R_parent_from_child`. `translation_m` is the child
    /// origin in the parent, metres. A non-finite translation is rejected
    /// before the rotation is stored.
    pub fn from_parts(
        rotation: Quaternion,
        translation_m: [f64; 3],
    ) -> Result<Self, GeometryError> {
        let [x, y, z] = translation_m;
        for (field, v) in [
            ("translation.x", x),
            ("translation.y", y),
            ("translation.z", z),
        ] {
            if !v.is_finite() {
                return Err(GeometryError::NonFinite { field });
            }
        }
        Ok(Self {
            rotation,
            translation_m,
        })
    }

    /// Rotation `R_parent_from_child`.
    pub fn rotation(self) -> Quaternion {
        self.rotation
    }

    /// Child origin in the parent, metres.
    pub fn translation_m(self) -> [f64; 3] {
        self.translation_m
    }

    /// Maps a point from the child frame into the parent: `R p + t`.
    pub fn apply(self, child_m: [f64; 3]) -> [f64; 3] {
        let [rx, ry, rz] = self.rotation.rotate(child_m);
        let [tx, ty, tz] = self.translation_m;
        [rx + tx, ry + ty, rz + tz]
    }

    /// `T_child_from_parent`.
    pub fn inverse(self) -> Self {
        let rotation = self.rotation.inverse();
        let [x, y, z] = rotation.rotate(self.translation_m);
        Self {
            rotation,
            translation_m: [-x, -y, -z],
        }
    }

    /// `T_parent_from_grandchild = self * child_from_grandchild`, where
    /// `self` is `T_parent_from_child`.
    pub fn compose(self, child_from_grandchild: Self) -> Self {
        let rotation = self.rotation * child_from_grandchild.rotation;
        let t = self.apply(child_from_grandchild.translation_m);
        Self {
            rotation,
            translation_m: t,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ROTATION_NORM_TOLERANCE;

    fn yaw_90() -> Quaternion {
        let h = std::f64::consts::FRAC_1_SQRT_2;
        Quaternion::from_xyzw(0.0, 0.0, h, h).unwrap()
    }

    #[test]
    fn identity_round_trip_is_exact() {
        let t = RigidTransform::identity();
        let p = [1.25, -3.5, 0.125];
        assert_eq!(t.apply(p), p);
        assert_eq!(t.inverse().apply(p), p);
        assert_eq!(t.compose(t).apply(p), p);
        assert_eq!(t.rotation(), Quaternion::identity());
        assert_eq!(t.translation_m(), [0.0, 0.0, 0.0]);
    }

    #[test]
    fn translation_is_checked_component_wise() {
        for (i, field) in ["translation.x", "translation.y", "translation.z"]
            .into_iter()
            .enumerate()
        {
            let mut c = [0.0, 0.0, 0.0];
            c[i] = f64::NAN;
            assert_eq!(
                RigidTransform::from_parts(Quaternion::identity(), c),
                Err(GeometryError::NonFinite { field })
            );
            c[i] = f64::INFINITY;
            assert_eq!(
                RigidTransform::from_parts(Quaternion::identity(), c),
                Err(GeometryError::NonFinite { field })
            );
        }
    }

    #[test]
    fn yaw_then_translate_matches_an_independent_matrix() {
        // Active +90° about z, then child origin at (10, 0, -2) m in the parent.
        // Matrix: x' = -y + 10, y' = x, z' = z - 2.
        let t = RigidTransform::from_parts(yaw_90(), [10.0, 0.0, -2.0]).unwrap();
        let p = t.apply([1.0, 0.0, 4.0]);
        assert!((p[0] - 10.0).abs() < 1e-12, "{p:?}");
        assert!((p[1] - 1.0).abs() < 1e-12, "{p:?}");
        assert!((p[2] - 2.0).abs() < 1e-12, "{p:?}");
        let back = t.inverse().apply(p);
        assert!((back[0] - 1.0).abs() < 1e-12, "{back:?}");
        assert!(back[1].abs() < 1e-12, "{back:?}");
        assert!((back[2] - 4.0).abs() < 1e-12, "{back:?}");
    }

    #[test]
    fn composing_two_yaws_is_180_degrees() {
        let yaw = RigidTransform::from_parts(yaw_90(), [0.0, 0.0, 0.0]).unwrap();
        let half_turn = yaw.compose(yaw);
        let p = half_turn.apply([1.0, 0.0, 0.0]);
        // 180° about z: (1, 0, 0) -> (-1, 0, 0).
        assert!((p[0] + 1.0).abs() < 1e-12, "{p:?}");
        assert!(p[1].abs() < 1e-12 && p[2].abs() < 1e-12, "{p:?}");
        let norm = {
            let q = half_turn.rotation().xyzw();
            (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt()
        };
        assert!((norm - 1.0).abs() < ROTATION_NORM_TOLERANCE, "{norm}");
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    fn yaw_pitch(yaw: f64, pitch: f64) -> Quaternion {
        let (sy, cy) = ((yaw / 2.0).sin(), (yaw / 2.0).cos());
        let (sp, cp) = ((pitch / 2.0).sin(), (pitch / 2.0).cos());
        let q_yaw = Quaternion::from_xyzw(0.0, 0.0, sy, cy).unwrap();
        let q_pitch = Quaternion::from_xyzw(0.0, sp, 0.0, cp).unwrap();
        let q = q_yaw * q_pitch;
        let [x, y, z, w] = q.xyzw();
        Quaternion::from_xyzw(x, y, z, w).unwrap()
    }

    proptest! {
        #[test]
        fn inverse_and_compose_round_trip(
            yaw in -3.0f64..3.0,
            pitch in -1.2f64..1.2,
            t in prop::array::uniform3(-1_000.0f64..1_000.0),
            p in prop::array::uniform3(-100.0f64..100.0),
            extra in prop::array::uniform3(-20.0f64..20.0),
        ) {
            let transform = RigidTransform::from_parts(yaw_pitch(yaw, pitch), t).unwrap();
            let back = transform.inverse().apply(transform.apply(p));
            for i in 0..3 {
                prop_assert!((back[i] - p[i]).abs() < 1e-8, "axis {i}: {back:?} vs {p:?}");
            }
            let shift = RigidTransform::from_parts(Quaternion::identity(), extra).unwrap();
            let via = transform.compose(shift).apply([0.0, 0.0, 0.0]);
            let direct = transform.apply(extra);
            for i in 0..3 {
                prop_assert!((via[i] - direct[i]).abs() < 1e-8);
            }
        }
    }
}
