# Coordinates, units, and limits

What is implemented today, followed by conventions that are specified but
not yet used in code.

## Geographic anchor

Each site has an immutable anchor (`GeodeticAnchor`, `verimap-domain`):

| Field | Unit | Accepted range |
| --- | --- | --- |
| `longitude_deg` | WGS84 degrees, east positive | −180 … 180 |
| `latitude_deg` | WGS84 degrees, north positive | −85 … 85 |
| `ellipsoidal_height_m` | metres above the WGS84 ellipsoid | −12 000 … 100 000 |

Values must be finite. A non-finite value is reported before any range
check. The height is **ellipsoidal**, not mean sea level and not height
above ground. The latitude limit keeps every site inside the 2D view's Web
Mercator range (±85.05°; see below).

## Scene frame

Scene coordinates form a right-handed **ENU** frame at the anchor: x east,
y north, z up, in **integer millimetres** (`i64`). The kernel guards apply:

- every coordinate lies in **[−10⁸, 10⁸] mm** (±100 km;
  `COORD_MIN_MM`/`COORD_MAX_MM`). Both ends are accepted;
- an axis interval `[min, max]` is closed, with `min ≤ max` allowed to be
  equal (`AxisBoundsMm`);
- `SceneBounds` additionally requires positive extent on every axis
  (`max > min`).

These guards are proved for the generated model; see
[verification.md](verification.md) rows 1a–1d. Millimetre storage is a
representation, not an accuracy claim.

The bundled **Synthetic Site Alpha** (`verimap-sim`, fictional) is anchored
at (−98.35°, 38.75°, 0 m ellipsoidal). Its bounds are x, y ∈ [−100 m, 100 m]
and z ∈ [−20 m, 80 m], a 200 m × 200 m site.

## Rendering

The browser converts scene millimetres to metres (`/ 1000`) and places them
with Cesium's `Transforms.eastNorthUpToFixedFrame` at the anchor. This is
JavaScript `number` (f64) arithmetic. Cesium uploads positions relative to
the eye or to a centre, so f32 GPU precision does not truncate Earth-centred
coordinates.

- **3D:** the scene-bounds box is drawn in true ENU. The reference ground is
  imagery draped on the ellipsoid over the longitude/latitude rectangle that
  contains the site's corners. At this site size, that rectangle and the
  ENU square differ by millimetres. Over areas tens of kilometres across,
  they would not.
- **2D:** Web Mercator, which is conformal, so a square site stays square.
  Cesium's default plate carrée would stretch it east–west by
  1/cos(latitude), 1.28× at the demo site. Web Mercator still varies in
  scale with latitude, so a distance read off the 2D screen is not a
  measurement. 2D does not show height.

## Specified, not yet implemented

These come from CLAUDE.md §5 and are listed here so the first code that uses
them matches:

- Sensor frames: named, versioned transforms `T_parent_from_child`;
  quaternions `[x, y, z, w]`, with invalid rotations rejected at a
  documented normalization tolerance. All transforms use f64.
- Surface chunks: 32 × 32 cells at 1 m. Cell and chunk indices will use
  floor division, so −1 mm falls in cell −1, not cell 0. The rounding rule
  for converting f64 metres to millimetres will be documented with the code.
- Confidence: integer basis points in [0, 10 000] (implemented in the
  kernel; not yet on the wire).
