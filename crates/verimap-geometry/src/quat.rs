//! Unit quaternions in `[x, y, z, w]` order. A value is stored exactly as
//! accepted: this module checks the norm and does not renormalize.

use crate::GeometryError;

/// How far a quaternion's Euclidean norm may sit from 1 and still be a rotation.
///
/// `1e-9` is a few thousand ulps around magnitude 1, so a quaternion that was
/// normalized in `f64` is accepted, and anything a caller forgot to normalize
/// is not. The check happens after non-finite components are rejected.
pub const ROTATION_NORM_TOLERANCE: f64 = 1e-9;

/// A rotation. Components are `[x, y, z, w]`, and the norm is within
/// [`ROTATION_NORM_TOLERANCE`] of 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quaternion {
    x: f64,
    y: f64,
    z: f64,
    w: f64,
}

impl Quaternion {
    /// The identity rotation, `(0, 0, 0, 1)`.
    pub fn identity() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        }
    }

    /// Accepts `[x, y, z, w]` when every component is finite and the norm is
    /// within [`ROTATION_NORM_TOLERANCE`] of 1. The stored value is the input,
    /// not a renormalized copy.
    pub fn from_xyzw(x: f64, y: f64, z: f64, w: f64) -> Result<Self, GeometryError> {
        for (field, v) in [("quat.x", x), ("quat.y", y), ("quat.z", z), ("quat.w", w)] {
            if !v.is_finite() {
                return Err(GeometryError::NonFinite { field });
            }
        }
        let norm_sq = x * x + y * y + z * z + w * w;
        let norm = norm_sq.sqrt();
        if (norm - 1.0).abs() > ROTATION_NORM_TOLERANCE {
            return Err(GeometryError::RotationNotUnit);
        }
        Ok(Self { x, y, z, w })
    }

    /// `[x, y, z, w]`.
    pub fn xyzw(self) -> [f64; 4] {
        [self.x, self.y, self.z, self.w]
    }

    /// `q⁻¹` for a unit quaternion: the conjugate. The norm is unchanged.
    pub fn inverse(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
            w: self.w,
        }
    }

    /// Rotates `v`, in the same length unit as `v`.
    pub fn rotate(self, v: [f64; 3]) -> [f64; 3] {
        // q * (v, 0) * q⁻¹. The scalar part of a unit rotation is 0 and is dropped.
        let [x, y, z] = v;
        let qv = Self { x, y, z, w: 0.0 };
        let r = self * qv * self.inverse();
        [r.x, r.y, r.z]
    }
}

impl core::ops::Mul for Quaternion {
    type Output = Self;

    /// Hamilton product. `T_a_from_c = T_a_from_b * T_b_from_c` multiplies in that order.
    fn mul(self, rhs: Self) -> Self {
        let (x1, y1, z1, w1) = (self.x, self.y, self.z, self.w);
        let (x2, y2, z2, w2) = (rhs.x, rhs.y, rhs.z, rhs.w);
        Self {
            x: w1 * x2 + x1 * w2 + y1 * z2 - z1 * y2,
            y: w1 * y2 - x1 * z2 + y1 * w2 + z1 * x2,
            z: w1 * z2 + x1 * y2 - y1 * x2 + z1 * w2,
            w: w1 * w2 - x1 * x2 - y1 * y2 - z1 * z2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_accepted_and_fixes_vectors() {
        let q = Quaternion::identity();
        assert_eq!(q.xyzw(), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(q.rotate([1.0, -2.0, 3.5]), [1.0, -2.0, 3.5]);
        assert_eq!(q.inverse().xyzw(), q.xyzw());
        assert_eq!((q * q).xyzw(), q.xyzw());
    }

    #[test]
    fn non_finite_components_are_rejected_before_the_norm() {
        for (i, field) in ["quat.x", "quat.y", "quat.z", "quat.w"]
            .into_iter()
            .enumerate()
        {
            for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                let mut c = [0.0, 0.0, 0.0, 1.0];
                c[i] = bad;
                assert_eq!(
                    Quaternion::from_xyzw(c[0], c[1], c[2], c[3]),
                    Err(GeometryError::NonFinite { field }),
                    "{field} {bad}"
                );
            }
        }
    }

    #[test]
    fn norm_tolerance_is_closed_and_does_not_renormalize() {
        // Stay off the rounding boundary of `1 ± tolerance` in f64.
        let inside = 1.0 + ROTATION_NORM_TOLERANCE * 0.5;
        let q = Quaternion::from_xyzw(0.0, 0.0, 0.0, inside).unwrap();
        assert_eq!(q.xyzw()[3], inside);
        let outside = 1.0 + ROTATION_NORM_TOLERANCE * 2.0;
        assert_eq!(
            Quaternion::from_xyzw(0.0, 0.0, 0.0, outside),
            Err(GeometryError::RotationNotUnit)
        );
        assert_eq!(
            Quaternion::from_xyzw(0.0, 0.0, 0.0, 0.0),
            Err(GeometryError::RotationNotUnit)
        );
        let low = 1.0 - ROTATION_NORM_TOLERANCE * 0.5;
        assert!(Quaternion::from_xyzw(0.0, 0.0, 0.0, -low).is_ok());
        assert_eq!(
            Quaternion::from_xyzw(0.0, 0.0, 0.0, -(1.0 - ROTATION_NORM_TOLERANCE * 2.0)),
            Err(GeometryError::RotationNotUnit)
        );
        // q and -q are the same turn.
        let neg = Quaternion::from_xyzw(0.0, 0.0, 0.0, -1.0).unwrap();
        assert_eq!(neg.rotate([3.0, 4.0, 5.0]), [3.0, 4.0, 5.0]);
    }

    #[test]
    fn yaw_90_degrees_matches_the_z_rotation_matrix() {
        // Right-handed, active rotation about z by +90°: (1, 0, 0) -> (0, 1, 0).
        // q = (0, 0, sin(θ/2), cos(θ/2)).
        let h = std::f64::consts::FRAC_1_SQRT_2;
        let q = Quaternion::from_xyzw(0.0, 0.0, h, h).unwrap();
        let p = q.rotate([1.0, 0.0, 0.0]);
        assert!((p[0] - 0.0).abs() < 1e-12, "{p:?}");
        assert!((p[1] - 1.0).abs() < 1e-12, "{p:?}");
        assert!((p[2] - 0.0).abs() < 1e-12, "{p:?}");
        // The matrix [0 -1 0; 1 0 0; 0 0 1] applied to (0, 1, 0) is (-1, 0, 0).
        let q2 = q.rotate([0.0, 1.0, 0.0]);
        assert!((q2[0] + 1.0).abs() < 1e-12 && q2[1].abs() < 1e-12, "{q2:?}");
        let back = q.inverse().rotate(p);
        assert!((back[0] - 1.0).abs() < 1e-12, "{back:?}");
    }
}
