//! Scene positions in integer millimetres, and the one place floating-point
//! metres are quantized to them.

use verimap_kernel::{COORD_MAX_MM, COORD_MIN_MM, coord_in_range};

use crate::DomainError;

/// A position in the scene frame (ENU from the anchor), in millimetres.
/// Every coordinate satisfies the kernel's [`coord_in_range`] guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScenePoint {
    x_mm: i64,
    y_mm: i64,
    z_mm: i64,
}

impl ScenePoint {
    /// Validates integer millimetre coordinates.
    pub fn new(x_mm: i64, y_mm: i64, z_mm: i64) -> Result<Self, DomainError> {
        for (field, v) in [
            ("position.x", x_mm),
            ("position.y", y_mm),
            ("position.z", z_mm),
        ] {
            if !coord_in_range(v) {
                return Err(DomainError::CoordinateOutOfRange { field });
            }
        }
        Ok(Self { x_mm, y_mm, z_mm })
    }

    /// Quantizes metres to millimetres with [`metres_to_mm`].
    pub fn from_metres(x_m: f64, y_m: f64, z_m: f64) -> Result<Self, DomainError> {
        Ok(Self {
            x_mm: metres_to_mm("position.x", x_m)?,
            y_mm: metres_to_mm("position.y", y_m)?,
            z_mm: metres_to_mm("position.z", z_m)?,
        })
    }

    /// East, millimetres.
    pub fn x_mm(&self) -> i64 {
        self.x_mm
    }

    /// North, millimetres.
    pub fn y_mm(&self) -> i64 {
        self.y_mm
    }

    /// Up, millimetres.
    pub fn z_mm(&self) -> i64 {
        self.z_mm
    }

    /// The same horizontal position at height `z_mm`.
    pub fn with_z(&self, z_mm: i64) -> Result<Self, DomainError> {
        Self::new(self.x_mm, self.y_mm, z_mm)
    }

    /// Squared horizontal distance to `other`, in mm². Exact: coordinates
    /// differ by at most 2·10⁸, so the sum of squares fits easily in `i128`.
    pub fn horizontal_distance_sq(&self, other: &ScenePoint) -> i128 {
        let dx = i128::from(self.x_mm) - i128::from(other.x_mm);
        let dy = i128::from(self.y_mm) - i128::from(other.y_mm);
        dx * dx + dy * dy
    }
}

/// Converts metres to millimetres, rounding half away from zero
/// (`f64::round`), and checks the result against the kernel coordinate
/// range. NaN and infinities are rejected before any arithmetic.
///
/// The conversion is exact for every result in range: `|mm| ≤ 10⁸ < 2⁵³`,
/// so the rounded `f64` is an integer the `i64` represents exactly. The
/// product `v * 1000.0` is one IEEE-754 operation, so native and WASM builds
/// agree bit for bit.
pub fn metres_to_mm(field: &'static str, v: f64) -> Result<i64, DomainError> {
    if !v.is_finite() {
        return Err(DomainError::NonFinite { field });
    }
    let mm = (v * 1000.0).round();
    // Both bounds are exactly representable in f64.
    if !(COORD_MIN_MM as f64 <= mm && mm <= COORD_MAX_MM as f64) {
        return Err(DomainError::CoordinateOutOfRange { field });
    }
    // In range and integral, as argued above, so the cast neither truncates
    // nor saturates.
    #[allow(clippy::cast_possible_truncation)]
    let mm = mm as i64;
    Ok(mm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantizes_half_away_from_zero() {
        assert_eq!(metres_to_mm("v", 0.0), Ok(0));
        assert_eq!(metres_to_mm("v", 1.2345), Ok(1235));
        assert_eq!(metres_to_mm("v", -1.2345), Ok(-1235));
        assert_eq!(metres_to_mm("v", 0.0004), Ok(0));
        assert_eq!(metres_to_mm("v", -0.0004), Ok(0));
        assert_eq!(metres_to_mm("v", 0.0005), Ok(1));
        assert_eq!(metres_to_mm("v", -0.0005), Ok(-1));
    }

    #[test]
    fn range_boundaries_and_non_finite() {
        assert_eq!(metres_to_mm("v", 100_000.0), Ok(COORD_MAX_MM));
        assert_eq!(metres_to_mm("v", -100_000.0), Ok(COORD_MIN_MM));
        // 100 km + 0.4 mm rounds back into range; + 0.6 mm does not.
        assert_eq!(metres_to_mm("v", 100_000.000_4), Ok(COORD_MAX_MM));
        assert_eq!(
            metres_to_mm("v", 100_000.000_6),
            Err(DomainError::CoordinateOutOfRange { field: "v" })
        );
        assert_eq!(
            metres_to_mm("v", 1e300),
            Err(DomainError::CoordinateOutOfRange { field: "v" })
        );
        for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                metres_to_mm("v", v),
                Err(DomainError::NonFinite { field: "v" })
            );
        }
    }

    #[test]
    fn integer_millimetres_round_trip_through_metres() {
        for mm in [0, 1, -1, 999, -999, 12_345_678, COORD_MAX_MM, COORD_MIN_MM] {
            let m = mm as f64 / 1000.0;
            assert_eq!(metres_to_mm("v", m), Ok(mm), "{mm}");
        }
    }

    #[test]
    fn points_validate_each_axis() {
        assert!(ScenePoint::new(COORD_MAX_MM, COORD_MIN_MM, 0).is_ok());
        assert_eq!(
            ScenePoint::new(0, COORD_MAX_MM + 1, 0),
            Err(DomainError::CoordinateOutOfRange {
                field: "position.y"
            })
        );
        assert_eq!(
            ScenePoint::from_metres(0.0, 0.0, f64::NAN),
            Err(DomainError::NonFinite {
                field: "position.z"
            })
        );
    }

    #[test]
    fn horizontal_distance_is_exact_at_the_extremes() {
        let a = ScenePoint::new(COORD_MIN_MM, COORD_MIN_MM, 0).unwrap();
        let b = ScenePoint::new(COORD_MAX_MM, COORD_MAX_MM, 0).unwrap();
        assert_eq!(a.horizontal_distance_sq(&b), 2 * 200_000_000_i128.pow(2));
    }
}
