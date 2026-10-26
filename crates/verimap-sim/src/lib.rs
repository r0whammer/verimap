//! Deterministic synthetic scenes for VeriMap; seeded sensors and the
//! simulation clock arrive with milestone M1.
//!
//! The simulator owns the hidden ground truth of its scenes. Normal
//! processing receives only what a simulated sensor reports and must never
//! read truth from this crate.

#![forbid(unsafe_code)]

use verimap_domain::{DomainError, GeodeticAnchor, SceneBounds, SiteFrame, SiteId};

/// Version of the bundled synthetic scene definitions. Bump it whenever a
/// scene changes, so recorded sessions can tell which scene they used.
pub const SCENE_VERSION: u32 = 1;

/// The bundled fictional demo site: a 200 m × 200 m outdoor area centred on
/// its anchor, from 20 m below to 80 m above it.
///
/// The anchor is an arbitrary rural point chosen only so that longitude and
/// latitude have distinct signs and magnitudes (a swap fails validation). It
/// has no survey meaning. Its height is 0 m above the WGS84 ellipsoid, so the
/// synthetic ground lies on the renderer's plain ellipsoid surface.
pub fn demo_site() -> Result<SiteFrame, DomainError> {
    SiteFrame::new(
        SiteId::new("demo-site-alpha")?,
        "Synthetic Site Alpha",
        GeodeticAnchor::new(-98.35, 38.75, 0.0)?,
        SceneBounds::new((-100_000, 100_000), (-100_000, 100_000), (-20_000, 80_000))?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_site_is_valid_and_200m_square() {
        let site = demo_site().expect("bundled site must validate");
        let b = site.bounds();
        assert_eq!(b.x().max() - b.x().min(), 200_000);
        assert_eq!(b.y().max() - b.y().min(), 200_000);
        assert_eq!(site.anchor().ellipsoidal_height_m(), 0.0);
    }
}
