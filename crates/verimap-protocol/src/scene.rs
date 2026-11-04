//! Scene DTOs: what the UI renders and inspects, converted from validated
//! domain values. Only the core produces these; the UI never sends them.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use verimap_domain::{
    Annotation, Cell, ChunkId, Entity, EntityClass, Freshness, HistoryEntry, ObservationRef,
    Placement, ScenePoint, SurfaceChunk, TrackFix,
};

use crate::U64String;
use crate::bridge::ErrorCode;

/// A scene position in integer millimetres (ENU from the anchor). Values
/// are within ±10⁸, so they are exact as JSON numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PointMmDto {
    /// East.
    #[schemars(range(min = -100000000, max = 100000000))]
    pub x_mm: i64,
    /// North.
    #[schemars(range(min = -100000000, max = 100000000))]
    pub y_mm: i64,
    /// Up.
    #[schemars(range(min = -100000000, max = 100000000))]
    pub z_mm: i64,
}

impl From<&ScenePoint> for PointMmDto {
    fn from(p: &ScenePoint) -> Self {
        Self {
            x_mm: p.x_mm(),
            y_mm: p.y_mm(),
            z_mm: p.z_mm(),
        }
    }
}

/// Believed class of a track: detector output, not a verified identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum EntityClassDto {
    Person,
    Animal,
    Vehicle,
    Building,
    Obstacle,
    Sensor,
    Unknown,
}

impl From<EntityClass> for EntityClassDto {
    fn from(c: EntityClass) -> Self {
        match c {
            EntityClass::Person => Self::Person,
            EntityClass::Animal => Self::Animal,
            EntityClass::Vehicle => Self::Vehicle,
            EntityClass::Building => Self::Building,
            EntityClass::Obstacle => Self::Obstacle,
            EntityClass::Sensor => Self::Sensor,
            EntityClass::Unknown => Self::Unknown,
        }
    }
}

/// How recently a track was observed, by the scene clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessDto {
    /// Observed within the last 3 s of scene time.
    Active,
    /// Not observed for more than 3 s.
    Stale,
    /// Not observed for more than 15 s; still a track until deleted.
    Expired,
}

impl From<Freshness> for FreshnessDto {
    fn from(f: Freshness) -> Self {
        match f {
            Freshness::Active => Self::Active,
            Freshness::Stale => Self::Stale,
            Freshness::Expired => Self::Expired,
        }
    }
}

/// Where an annotation's height came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlacementDto {
    /// On the scene's ground plane, z = 0 m: an assumed height.
    GroundPlane,
}

impl From<Placement> for PlacementDto {
    fn from(p: Placement) -> Self {
        match p {
            Placement::GroundPlane => Self::GroundPlane,
        }
    }
}

impl From<PlacementDto> for Placement {
    fn from(p: PlacementDto) -> Self {
        match p {
            PlacementDto::GroundPlane => Self::GroundPlane,
        }
    }
}

/// The observation a state came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceRefDto {
    /// Reporting source.
    pub source_id: String,
    /// The source's observation sequence.
    pub sequence: U64String,
}

impl From<&ObservationRef> for SourceRefDto {
    fn from(r: &ObservationRef) -> Self {
        Self {
            source_id: r.source.as_str().to_owned(),
            sequence: r.sequence.into(),
        }
    }
}

/// A track as the map shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityDto {
    /// Stable track ID, `namespace:local`. Not a real-world identity.
    pub id: String,
    /// Believed class.
    pub class: EntityClassDto,
    /// Latest position.
    pub position: PointMmDto,
    /// Horizontal 1σ uncertainty, millimetres.
    pub sigma_mm: u32,
    /// Detector confidence, basis points (0–10000). Not a calibrated
    /// probability.
    #[schemars(range(max = 10000))]
    pub confidence_bp: u16,
    /// Freshness at the view's scene time.
    pub freshness: FreshnessDto,
    /// Acquisition tick of the latest fix.
    pub last_observed_tick: U64String,
    /// Sequence of the event that last changed the track.
    pub revision: U64String,
    /// Source of the latest fix.
    pub source: SourceRefDto,
}

impl EntityDto {
    /// The DTO of `entity` at `revision`, with freshness at tick `now`.
    pub fn new(entity: &Entity, revision: u64, now: u64) -> Self {
        let latest = entity.latest();
        Self {
            id: entity.id().as_str().to_owned(),
            class: entity.class().into(),
            position: (&latest.position).into(),
            sigma_mm: latest.uncertainty.sigma_mm(),
            confidence_bp: latest.confidence.basis_points(),
            freshness: entity.freshness(now).into(),
            last_observed_tick: entity.last_observed_tick().into(),
            revision: revision.into(),
            source: entity.source().into(),
        }
    }
}

/// One past fix in a track's history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FixDto {
    /// Acquisition tick.
    pub acquired_tick: U64String,
    /// Measured position.
    pub position: PointMmDto,
    /// Horizontal 1σ uncertainty, millimetres.
    pub sigma_mm: u32,
    /// Detector confidence, basis points.
    #[schemars(range(max = 10000))]
    pub confidence_bp: u16,
    /// Source observation.
    pub source: SourceRefDto,
    /// Sequence of the event that recorded it.
    pub revision: U64String,
}

impl From<&HistoryEntry> for FixDto {
    fn from(h: &HistoryEntry) -> Self {
        let TrackFix {
            acquired_tick,
            position,
            uncertainty,
            confidence,
        } = h.fix;
        Self {
            acquired_tick: acquired_tick.into(),
            position: (&position).into(),
            sigma_mm: uncertainty.sigma_mm(),
            confidence_bp: confidence.basis_points(),
            source: (&h.source).into(),
            revision: h.revision.into(),
        }
    }
}

/// A track with its observation history, for the detail panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityDetailDto {
    /// Current state.
    pub entity: EntityDto,
    /// Most recent fixes, oldest first (at most 16).
    pub history: Vec<FixDto>,
}

/// An operator annotation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnnotationDto {
    /// Stable ID, `namespace:local`.
    pub id: String,
    /// Label. Untrusted: render as text only.
    pub label: String,
    /// Marker position.
    pub position: PointMmDto,
    /// Where its height came from.
    pub placement: PlacementDto,
    /// Sequence of the event that last changed it; edits must name it.
    pub revision: U64String,
}

impl AnnotationDto {
    /// The DTO of `annotation` at `revision`.
    pub fn new(annotation: &Annotation, revision: u64) -> Self {
        Self {
            id: annotation.id().as_str().to_owned(),
            label: annotation.label().to_owned(),
            position: annotation.position().into(),
            placement: annotation.placement().into(),
            revision: revision.into(),
        }
    }
}

/// Whether the view shows the live session or a replayed past state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ViewModeDto {
    /// The live session; edits and simulation are allowed.
    Live,
    /// A state rebuilt from the journal; read-only. The live session is
    /// paused and unchanged.
    Review,
}

/// Where the view is in the session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimelineDto {
    /// Live or review.
    pub mode: ViewModeDto,
    /// Scene tick shown.
    pub tick: U64String,
    /// Last event sequence included in the view.
    pub sequence: U64String,
    /// Live scene tick.
    pub live_tick: U64String,
    /// Last committed sequence.
    pub live_sequence: U64String,
    /// Events in the journal.
    pub journal_events: U64String,
    /// Configured journal bound, in events.
    pub journal_limit: U64String,
}

/// Observation counts since the session started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IngestStatsDto {
    /// Delivered by sources.
    pub received: U64String,
    /// Committed as track events.
    pub accepted: U64String,
    /// Rejected by validation.
    pub rejected: U64String,
    /// Duplicates of accepted observations.
    pub duplicates: U64String,
    /// Older than the matched track's latest fix; not applied.
    pub late: U64String,
}

/// One reconstructed cell. Unknown cells have `known: false`, height 0, and
/// occupancy 0. That is not the same as an observed empty cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CellDto {
    /// Height above the anchor, millimetres. Zero when unknown.
    #[schemars(range(min = -100000000, max = 100000000))]
    pub height_mm: i32,
    /// Occupancy in basis points. Zero when unknown or observed empty.
    #[schemars(range(max = 10000))]
    pub occupancy_bp: u16,
    /// Whether a sensor has reported this cell.
    pub known: bool,
}

impl From<Cell> for CellDto {
    fn from(cell: Cell) -> Self {
        Self {
            height_mm: cell.height_mm(),
            occupancy_bp: cell.occupancy_bp(),
            known: cell.is_known(),
        }
    }
}

/// A 32 × 32 surface chunk in the scene frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChunkDto {
    /// `ix,iy`, the key the view uses to upsert the chunk.
    pub id: String,
    /// Easting chunk index. Negative is west of the anchor.
    pub ix: i32,
    /// Northing chunk index.
    pub iy: i32,
    /// Sequence of the event that wrote the chunk.
    pub revision: U64String,
    /// Row-major cells, 32 × 32. A height grid does not represent overhangs.
    pub cells: Vec<CellDto>,
}

impl ChunkDto {
    /// The DTO of `chunk` at `revision`.
    pub fn new(chunk: &SurfaceChunk, revision: u64) -> Self {
        let id = chunk.id();
        Self {
            id: chunk_key(id),
            ix: id.ix(),
            iy: id.iy(),
            revision: revision.into(),
            cells: chunk.cells().iter().copied().map(CellDto::from).collect(),
        }
    }
}

/// `ix,iy` text for a chunk id.
pub fn chunk_key(id: ChunkId) -> String {
    format!("{},{}", id.ix(), id.iy())
}

/// A change to what the view shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneUpdateDto {
    /// When true, the lists below are the complete view: discard
    /// everything else. When false, they are changes to apply.
    pub replace: bool,
    /// Where the view is.
    pub timeline: TimelineDto,
    /// Observation counts (live session).
    pub stats: IngestStatsDto,
    /// Tracks added or changed.
    pub entities: Vec<EntityDto>,
    /// Track IDs no longer present (deleted, or absent in a review state).
    pub removed_entities: Vec<String>,
    /// Annotations added or changed.
    pub annotations: Vec<AnnotationDto>,
    /// Annotation IDs no longer present.
    pub removed_annotations: Vec<String>,
    /// Surface chunks added or replaced.
    pub chunks: Vec<ChunkDto>,
    /// Chunk ids no longer present, `ix,iy`.
    pub removed_chunks: Vec<String>,
    /// Why the request stopped early, when the changes above are only part
    /// of what was asked (for example a full journal stops the simulation).
    pub problem: Option<ProblemDto>,
}

/// A problem reported alongside a partial result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProblemDto {
    /// Machine-readable reason.
    pub code: ErrorCode,
    /// Human-readable detail. Render as text only.
    pub message: String,
}

/// Versions and parameters of the running session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionDto {
    /// Simulation seed.
    pub seed: U64String,
    /// Simulator behavior version.
    pub simulator_version: u32,
    /// Synthetic scene version.
    pub scene_version: u32,
    /// Committed-event format and reducer version.
    pub event_schema_version: u16,
    /// Scene time per tick, milliseconds.
    pub tick_ms: u32,
    /// Ticks without observation before a track is stale.
    pub stale_after_ticks: u32,
    /// Ticks without observation before a track is expired.
    pub expire_after_ticks: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use verimap_domain::{EntityClass, Freshness, Placement};

    #[test]
    fn class_and_freshness_use_stable_names() {
        for class in EntityClass::ALL {
            let dto = EntityClassDto::from(class);
            let json = serde_json::to_string(&dto).unwrap();
            assert_eq!(json, format!("\"{}\"", class.as_str()));
            assert_eq!(serde_json::from_str::<EntityClassDto>(&json).unwrap(), dto);
        }
        for (freshness, name) in [
            (Freshness::Active, "active"),
            (Freshness::Stale, "stale"),
            (Freshness::Expired, "expired"),
        ] {
            let dto = FreshnessDto::from(freshness);
            assert_eq!(serde_json::to_string(&dto).unwrap(), format!("\"{name}\""));
        }
        assert_eq!(
            Placement::from(PlacementDto::from(Placement::GroundPlane)),
            Placement::GroundPlane
        );
    }

    #[test]
    fn chunk_dto_copies_every_cell() {
        use verimap_domain::{CELL_COUNT, Cell, ChunkId, SCENE_FRAME, SurfaceChunk};
        let mut cells = vec![Cell::unknown(); CELL_COUNT];
        cells[1] = Cell::observed(1_500, 0).unwrap();
        let chunk =
            SurfaceChunk::from_cells(ChunkId::new(0, -1).unwrap(), SCENE_FRAME, &cells).unwrap();
        let dto = ChunkDto::new(&chunk, 7);
        assert_eq!(dto.id, "0,-1");
        assert_eq!(dto.ix, 0);
        assert_eq!(dto.iy, -1);
        assert_eq!(dto.revision.0, 7);
        assert!(!dto.cells[0].known);
        assert_eq!(dto.cells[1].height_mm, 1_500);
        assert_eq!(dto.cells[1].occupancy_bp, 0);
        assert!(dto.cells[1].known);
        assert_eq!(dto.cells.len(), CELL_COUNT);
    }
}
