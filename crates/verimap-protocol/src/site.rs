//! Site DTOs and their conversions to and from `verimap_domain::SiteFrame`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use verimap_domain::{AxisBoundsMm, DomainError, GeodeticAnchor, SceneBounds, SiteFrame, SiteId};

/// A site's identity, anchor, and scene bounds (docs/coordinates.md).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SiteDto {
    /// Lowercase slug, at most 64 bytes.
    #[schemars(
        length(min = 1, max = 64),
        regex(pattern = r"^[a-z0-9]([a-z0-9-]*[a-z0-9])?$")
    )]
    pub site_id: String,
    /// Display name, at most 128 bytes. Untrusted: render as text only.
    #[schemars(length(min = 1, max = 128))]
    pub name: String,
    /// Origin of the ENU scene frame.
    pub anchor: GeodeticAnchorDto,
    /// Region of the scene frame the site occupies.
    pub bounds: SceneBoundsDto,
}

/// A WGS84 position. Height is above the ellipsoid, not mean sea level.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GeodeticAnchorDto {
    /// Degrees, east positive.
    #[schemars(range(min = -180.0, max = 180.0))]
    pub longitude_deg: f64,
    /// Degrees, north positive; sites are limited to ±85°.
    #[schemars(range(min = -85.0, max = 85.0))]
    pub latitude_deg: f64,
    /// Metres above the WGS84 ellipsoid.
    #[schemars(range(min = -12000.0, max = 100000.0))]
    pub ellipsoidal_height_m: f64,
}

/// Per-axis scene bounds in millimetres: x east, y north, z up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneBoundsDto {
    /// East axis.
    pub x: AxisBoundsDto,
    /// North axis.
    pub y: AxisBoundsDto,
    /// Up axis.
    pub z: AxisBoundsDto,
}

/// A closed interval of scene coordinates in integer millimetres.
///
/// Values are within ±10⁸, so they are exact as JSON numbers in JavaScript.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AxisBoundsDto {
    /// Lower end, inclusive.
    #[schemars(range(min = -100000000, max = 100000000))]
    pub min_mm: i64,
    /// Upper end, inclusive.
    #[schemars(range(min = -100000000, max = 100000000))]
    pub max_mm: i64,
}

impl From<&SiteFrame> for SiteDto {
    fn from(site: &SiteFrame) -> Self {
        let axis = |a: AxisBoundsMm| AxisBoundsDto {
            min_mm: a.min(),
            max_mm: a.max(),
        };
        let (anchor, bounds) = (site.anchor(), site.bounds());
        Self {
            site_id: site.id().as_str().to_owned(),
            name: site.name().to_owned(),
            anchor: GeodeticAnchorDto {
                longitude_deg: anchor.longitude_deg(),
                latitude_deg: anchor.latitude_deg(),
                ellipsoidal_height_m: anchor.ellipsoidal_height_m(),
            },
            bounds: SceneBoundsDto {
                x: axis(bounds.x()),
                y: axis(bounds.y()),
                z: axis(bounds.z()),
            },
        }
    }
}

impl TryFrom<&SiteDto> for SiteFrame {
    type Error = DomainError;

    fn try_from(dto: &SiteDto) -> Result<Self, DomainError> {
        let pair = |a: AxisBoundsDto| (a.min_mm, a.max_mm);
        SiteFrame::new(
            SiteId::new(&dto.site_id)?,
            &dto.name,
            GeodeticAnchor::new(
                dto.anchor.longitude_deg,
                dto.anchor.latitude_deg,
                dto.anchor.ellipsoidal_height_m,
            )?,
            SceneBounds::new(pair(dto.bounds.x), pair(dto.bounds.y), pair(dto.bounds.z))?,
        )
    }
}
