//! The site frame: the immutable WGS84 anchor of a session's local scene
//! frame and the bounded region of that frame the scene may occupy.
//!
//! Scene coordinates are right-handed ENU (x east, y north, z up) in integer
//! millimetres from the anchor; see docs/coordinates.md.

use verimap_kernel::AxisBoundsMm;

use crate::{DomainError, ScenePoint, validate_label, validate_slug};

/// A WGS84 geodetic position: the origin of a session's ENU scene frame.
///
/// Height is ellipsoidal (above the WGS84 ellipsoid), never mean sea level
/// or height above ground.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeodeticAnchor {
    longitude_deg: f64,
    latitude_deg: f64,
    ellipsoidal_height_m: f64,
}

impl GeodeticAnchor {
    /// Largest accepted |latitude|. A local ENU frame and the 2D map view
    /// both degenerate toward the poles; polar sites need another design.
    pub const MAX_ABS_LATITUDE_DEG: f64 = 85.0;
    /// Lowest accepted ellipsoidal height (below the deepest ocean trench).
    pub const MIN_HEIGHT_M: f64 = -12_000.0;
    /// Highest accepted ellipsoidal height.
    pub const MAX_HEIGHT_M: f64 = 100_000.0;

    /// Validates an anchor: finite values, longitude in [−180, 180],
    /// latitude in [−85, 85], height in [−12 km, 100 km].
    pub fn new(
        longitude_deg: f64,
        latitude_deg: f64,
        ellipsoidal_height_m: f64,
    ) -> Result<Self, DomainError> {
        finite_within("anchor.longitude_deg", longitude_deg, -180.0, 180.0)?;
        finite_within(
            "anchor.latitude_deg",
            latitude_deg,
            -Self::MAX_ABS_LATITUDE_DEG,
            Self::MAX_ABS_LATITUDE_DEG,
        )?;
        finite_within(
            "anchor.ellipsoidal_height_m",
            ellipsoidal_height_m,
            Self::MIN_HEIGHT_M,
            Self::MAX_HEIGHT_M,
        )?;
        Ok(Self {
            longitude_deg,
            latitude_deg,
            ellipsoidal_height_m,
        })
    }

    /// Longitude in degrees, east positive.
    pub fn longitude_deg(&self) -> f64 {
        self.longitude_deg
    }

    /// Latitude in degrees, north positive.
    pub fn latitude_deg(&self) -> f64 {
        self.latitude_deg
    }

    /// Height above the WGS84 ellipsoid in metres.
    pub fn ellipsoidal_height_m(&self) -> f64 {
        self.ellipsoidal_height_m
    }
}

fn finite_within(field: &'static str, v: f64, min: f64, max: f64) -> Result<(), DomainError> {
    if !v.is_finite() {
        return Err(DomainError::NonFinite { field });
    }
    if v < min || v > max {
        return Err(DomainError::OutOfRange { field });
    }
    Ok(())
}

/// The region of the scene frame a site may occupy, per axis, in millimetres.
#[derive(Clone, Copy)]
pub struct SceneBounds {
    x: AxisBoundsMm,
    y: AxisBoundsMm,
    z: AxisBoundsMm,
}

impl SceneBounds {
    /// Validates `(min, max)` pairs for x (east), y (north), and z (up).
    /// Each axis must pass the kernel's interval guard and, for a site, have
    /// positive extent.
    pub fn new(x: (i64, i64), y: (i64, i64), z: (i64, i64)) -> Result<Self, DomainError> {
        Ok(Self {
            x: site_axis("bounds.x", x)?,
            y: site_axis("bounds.y", y)?,
            z: site_axis("bounds.z", z)?,
        })
    }

    /// East axis.
    pub fn x(&self) -> AxisBoundsMm {
        self.x
    }

    /// North axis.
    pub fn y(&self) -> AxisBoundsMm {
        self.y
    }

    /// Up axis.
    pub fn z(&self) -> AxisBoundsMm {
        self.z
    }

    /// Whether `p` lies inside the bounds on every axis (closed intervals).
    pub fn contains(&self, p: &ScenePoint) -> bool {
        self.x.contains(p.x_mm()) && self.y.contains(p.y_mm()) && self.z.contains(p.z_mm())
    }
}

impl core::fmt::Debug for SceneBounds {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let axis = |b: AxisBoundsMm| (b.min(), b.max());
        f.debug_struct("SceneBounds")
            .field("x", &axis(self.x))
            .field("y", &axis(self.y))
            .field("z", &axis(self.z))
            .finish()
    }
}

impl PartialEq for SceneBounds {
    fn eq(&self, other: &Self) -> bool {
        let same = |a: AxisBoundsMm, b: AxisBoundsMm| a.min() == b.min() && a.max() == b.max();
        same(self.x, other.x) && same(self.y, other.y) && same(self.z, other.z)
    }
}

fn site_axis(field: &'static str, (min, max): (i64, i64)) -> Result<AxisBoundsMm, DomainError> {
    let axis = AxisBoundsMm::new(min, max).map_err(|e| DomainError::from_kernel(field, e))?;
    if axis.min() == axis.max() {
        return Err(DomainError::EmptyExtent { field });
    }
    Ok(axis)
}

/// Identifier of a site: a lowercase slug of at most [`SiteId::MAX_BYTES`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SiteId(String);

impl SiteId {
    /// Longest accepted identifier, in bytes.
    pub const MAX_BYTES: usize = 64;

    /// Validates a slug identifier (see [`validate_slug`]).
    pub fn new(id: &str) -> Result<Self, DomainError> {
        validate_slug("site_id", id, Self::MAX_BYTES)?;
        Ok(Self(id.to_owned()))
    }

    /// The identifier text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A site: its identity, display name, scene-frame anchor, and bounds.
#[derive(Debug, Clone, PartialEq)]
pub struct SiteFrame {
    id: SiteId,
    name: String,
    anchor: GeodeticAnchor,
    bounds: SceneBounds,
}

impl SiteFrame {
    /// Longest accepted display name, in bytes.
    pub const NAME_MAX_BYTES: usize = 128;

    /// Assembles a site from validated parts and a display name.
    pub fn new(
        id: SiteId,
        name: &str,
        anchor: GeodeticAnchor,
        bounds: SceneBounds,
    ) -> Result<Self, DomainError> {
        validate_label("name", name, Self::NAME_MAX_BYTES)?;
        Ok(Self {
            id,
            name: name.to_owned(),
            anchor,
            bounds,
        })
    }

    /// Site identifier.
    pub fn id(&self) -> &SiteId {
        &self.id
    }

    /// Display name; untrusted text, render as text only.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Origin of the scene frame.
    pub fn anchor(&self) -> &GeodeticAnchor {
        &self.anchor
    }

    /// Region of the scene frame the site occupies.
    pub fn bounds(&self) -> &SceneBounds {
        &self.bounds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_accepts_boundaries() {
        assert!(GeodeticAnchor::new(-180.0, -85.0, -12_000.0).is_ok());
        assert!(GeodeticAnchor::new(180.0, 85.0, 100_000.0).is_ok());
        assert!(GeodeticAnchor::new(0.0, 0.0, 0.0).is_ok());
    }

    #[test]
    fn anchor_rejects_non_finite_before_range() {
        for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                GeodeticAnchor::new(v, 0.0, 0.0),
                Err(DomainError::NonFinite {
                    field: "anchor.longitude_deg"
                })
            );
            assert_eq!(
                GeodeticAnchor::new(0.0, v, 0.0),
                Err(DomainError::NonFinite {
                    field: "anchor.latitude_deg"
                })
            );
            assert_eq!(
                GeodeticAnchor::new(0.0, 0.0, v),
                Err(DomainError::NonFinite {
                    field: "anchor.ellipsoidal_height_m"
                })
            );
        }
    }

    #[test]
    fn anchor_rejects_out_of_range() {
        let field = |r: Result<GeodeticAnchor, DomainError>| r.map_err(|e| e.field());
        assert_eq!(
            field(GeodeticAnchor::new(180.000_001, 0.0, 0.0)),
            Err("anchor.longitude_deg")
        );
        assert_eq!(
            field(GeodeticAnchor::new(0.0, 85.000_001, 0.0)),
            Err("anchor.latitude_deg")
        );
        assert_eq!(
            field(GeodeticAnchor::new(0.0, -90.0, 0.0)),
            Err("anchor.latitude_deg")
        );
        assert_eq!(
            field(GeodeticAnchor::new(0.0, 0.0, 100_000.1)),
            Err("anchor.ellipsoidal_height_m")
        );
        // A swapped longitude/latitude pair with |lon| > 90 is caught.
        assert_eq!(
            field(GeodeticAnchor::new(38.75, -98.35, 0.0)),
            Err("anchor.latitude_deg")
        );
    }

    #[test]
    fn bounds_report_the_failing_axis() {
        let ok = (-1_000, 1_000);
        assert!(SceneBounds::new(ok, ok, ok).is_ok());
        assert_eq!(
            SceneBounds::new((5, -5), ok, ok).map(|_| ()),
            Err(DomainError::BoundsInverted { field: "bounds.x" })
        );
        assert_eq!(
            SceneBounds::new(ok, (0, 100_000_001), ok).map(|_| ()),
            Err(DomainError::CoordinateOutOfRange { field: "bounds.y" })
        );
        assert_eq!(
            SceneBounds::new(ok, ok, (7, 7)).map(|_| ()),
            Err(DomainError::EmptyExtent { field: "bounds.z" })
        );
    }

    #[test]
    fn site_validates_name() {
        let parts = || {
            (
                SiteId::new("s").unwrap(),
                GeodeticAnchor::new(1.0, 2.0, 3.0).unwrap(),
                SceneBounds::new((-1, 1), (-1, 1), (-1, 1)).unwrap(),
            )
        };
        let (id, anchor, bounds) = parts();
        assert!(SiteFrame::new(id, "Site", anchor, bounds).is_ok());
        let (id, anchor, bounds) = parts();
        assert!(SiteFrame::new(id, "bad\nname", anchor, bounds).is_err());
        let (id, anchor, bounds) = parts();
        let long = "x".repeat(SiteFrame::NAME_MAX_BYTES + 1);
        assert!(SiteFrame::new(id, &long, anchor, bounds).is_err());
    }
}
