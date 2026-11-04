//! Reconstructed surface chunks: 32 × 32 cells at 1 m.
//!
//! Height, occupancy, and unknown coverage are separate fields. A height grid
//! does not represent overhangs. Indexing uses the kernel's floor division,
//! so −1 mm is cell −1, and that cell is the last cell of chunk −1.
//! `docs/coordinates.md` records the rule.

use verimap_kernel::{
    CELL_INDEX_MAX, CELL_INDEX_MIN, CELL_SIZE_MM, CELLS_PER_CHUNK, cell_index, chunk_cell,
};

use crate::{Confidence, DomainError, SceneBounds};

/// Cells along one edge of a chunk.
pub const CHUNK_EDGE: usize = 32;
/// Cells in one chunk.
pub const CELL_COUNT: usize = CHUNK_EDGE * CHUNK_EDGE;
/// The only frame a chunk may name until sensor frames are wired through.
pub const SCENE_FRAME: &str = "scene";

/// Identity of one chunk in the scene frame. Independent of the revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkId {
    ix: i32,
    iy: i32,
}

impl ChunkId {
    /// Accepts indices that [`chunk_cell`] can produce from a [`cell_index`].
    pub fn new(ix: i32, iy: i32) -> Result<Self, DomainError> {
        if !chunk_index_in_range(ix) || !chunk_index_in_range(iy) {
            return Err(DomainError::CoordinateOutOfRange { field: "chunk" });
        }
        Ok(Self { ix, iy })
    }

    /// Chunk that contains the scene point `(x_mm, y_mm)`.
    pub fn containing(x_mm: i64, y_mm: i64) -> Result<Self, DomainError> {
        let cx = cell_index(x_mm).map_err(|e| DomainError::from_kernel("position.x", e))?;
        let cy = cell_index(y_mm).map_err(|e| DomainError::from_kernel("position.y", e))?;
        let x = chunk_cell(cx).map_err(|e| DomainError::from_kernel("position.x", e))?;
        let y = chunk_cell(cy).map_err(|e| DomainError::from_kernel("position.y", e))?;
        let ix = i32::try_from(x.chunk())
            .map_err(|_| DomainError::CoordinateOutOfRange { field: "chunk.ix" })?;
        let iy = i32::try_from(y.chunk())
            .map_err(|_| DomainError::CoordinateOutOfRange { field: "chunk.iy" })?;
        Self::new(ix, iy)
    }

    /// Easting index.
    pub fn ix(self) -> i32 {
        self.ix
    }

    /// Northing index.
    pub fn iy(self) -> i32 {
        self.iy
    }

    /// West edge of the chunk, millimetres. The chunk covers
    /// `[origin, origin + 32 m)` on each axis.
    pub fn origin_x_mm(self) -> i64 {
        i64::from(self.ix) * CELLS_PER_CHUNK * CELL_SIZE_MM
    }

    /// South edge of the chunk, millimetres.
    pub fn origin_y_mm(self) -> i64 {
        i64::from(self.iy) * CELLS_PER_CHUNK * CELL_SIZE_MM
    }

    /// Whether the chunk's half-open footprint meets `bounds` on x and y.
    pub fn intersects_xy(self, bounds: &SceneBounds) -> bool {
        let hit = |origin: i64, min: i64, max: i64| {
            let end = origin + CELLS_PER_CHUNK * CELL_SIZE_MM;
            origin <= max && end > min
        };
        hit(self.origin_x_mm(), bounds.x().min(), bounds.x().max())
            && hit(self.origin_y_mm(), bounds.y().min(), bounds.y().max())
    }
}

fn chunk_index_in_range(index: i32) -> bool {
    let Ok(lo) = chunk_cell(CELL_INDEX_MIN) else {
        return false;
    };
    let Ok(hi) = chunk_cell(CELL_INDEX_MAX) else {
        return false;
    };
    i64::from(index) >= lo.chunk() && i64::from(index) <= hi.chunk()
}

/// One cell. Unknown coverage, occupancy, and height are distinct: an unknown
/// cell stores height 0 and occupancy 0 and is not an observed empty cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    height_mm: i32,
    occupancy_bp: u16,
    known: bool,
}

impl Cell {
    /// Not yet observed.
    pub const fn unknown() -> Self {
        Self {
            height_mm: 0,
            occupancy_bp: 0,
            known: false,
        }
    }

    /// Observed empty at `height_mm`, or occupied when `occupancy_bp` is not 0.
    pub fn observed(height_mm: i32, occupancy_bp: u16) -> Result<Self, DomainError> {
        Self {
            height_mm,
            occupancy_bp,
            known: true,
        }
        .check()
    }

    /// Height above the anchor, millimetres. Zero when the cell is unknown.
    pub fn height_mm(self) -> i32 {
        self.height_mm
    }

    /// Occupancy in basis points, `0..=10000`. Zero when unknown or observed empty.
    pub fn occupancy_bp(self) -> u16 {
        self.occupancy_bp
    }

    /// Whether a sensor has reported this cell.
    pub fn is_known(self) -> bool {
        self.known
    }

    fn check(self) -> Result<Self, DomainError> {
        if !self.known {
            if self.height_mm != 0 || self.occupancy_bp != 0 {
                return Err(DomainError::OutOfRange {
                    field: "chunk.cell",
                });
            }
            return Ok(self);
        }
        let height = i64::from(self.height_mm);
        if cell_index(height).is_err() {
            return Err(DomainError::CoordinateOutOfRange {
                field: "chunk.height",
            });
        }
        Confidence::new(self.occupancy_bp).map_err(|e| {
            // Confidence::new only fails above 10 000; retarget the field.
            match e {
                DomainError::ConfidenceOutOfRange { .. } => DomainError::ConfidenceOutOfRange {
                    field: "chunk.occupancy",
                },
                other => other,
            }
        })?;
        Ok(self)
    }
}

/// One committed surface chunk. Only [`SurfaceChunk::from_cells`] builds one,
/// after every cell has been checked. A bad cell does not yield a partial chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceChunk {
    id: ChunkId,
    frame: String,
    cells: Box<[Cell; CELL_COUNT]>,
}

impl SurfaceChunk {
    /// Validates `frame` and all `cells` before building the chunk.
    /// `cells` must have length [`CELL_COUNT`]. `frame` must be [`SCENE_FRAME`].
    pub fn from_cells(id: ChunkId, frame: &str, cells: &[Cell]) -> Result<Self, DomainError> {
        if frame != SCENE_FRAME {
            return Err(DomainError::InvalidText {
                field: "chunk.frame",
                reason: "only the scene frame is accepted",
            });
        }
        if cells.len() != CELL_COUNT {
            return Err(DomainError::OutOfRange {
                field: "chunk.cells",
            });
        }
        let mut checked = [Cell::unknown(); CELL_COUNT];
        for (cell, out) in cells.iter().zip(checked.iter_mut()) {
            *out = cell.check()?;
        }
        Ok(Self {
            id,
            frame: frame.to_owned(),
            cells: Box::new(checked),
        })
    }

    /// Chunk identity.
    pub fn id(&self) -> ChunkId {
        self.id
    }

    /// Frame the chunk is expressed in.
    pub fn frame(&self) -> &str {
        &self.frame
    }

    /// Cells in row-major order, local northing major, local easting minor.
    pub fn cells(&self) -> &[Cell; CELL_COUNT] {
        &self.cells
    }

    /// The cell at local `(east, north)`, each in `0..32`.
    pub fn cell(&self, east: u8, north: u8) -> Option<&Cell> {
        let index = cell_offset(east, north)?;
        self.cells.get(index)
    }
}

fn cell_offset(east: u8, north: u8) -> Option<usize> {
    if usize::from(east) >= CHUNK_EDGE || usize::from(north) >= CHUNK_EDGE {
        return None;
    }
    Some(usize::from(north) * CHUNK_EDGE + usize::from(east))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_millimetres_fall_in_the_previous_chunk() {
        // −1 mm is cell −1, the last cell of chunk −1.
        let id = ChunkId::containing(-1, -1).unwrap();
        assert_eq!((id.ix(), id.iy()), (-1, -1));
        assert_eq!(id.origin_x_mm(), -32_000);
        // 32 m is the first millimetre of chunk 1; 32 m − 1 mm stays in chunk 0.
        assert_eq!(ChunkId::containing(31_999, 0).unwrap().ix(), 0);
        assert_eq!(ChunkId::containing(32_000, 0).unwrap().ix(), 1);
        assert_eq!(ChunkId::containing(0, -32_000).unwrap().iy(), -1);
        assert_eq!(ChunkId::containing(0, -32_001).unwrap().iy(), -2);
    }

    #[test]
    fn payloads_are_checked_in_full() {
        let id = ChunkId::new(0, 0).unwrap();
        assert!(SurfaceChunk::from_cells(id, SCENE_FRAME, &[]).is_err());
        let mut too_many = vec![Cell::unknown(); CELL_COUNT + 1];
        too_many[CELL_COUNT] = Cell::unknown();
        assert!(SurfaceChunk::from_cells(id, SCENE_FRAME, &too_many).is_err());
        let mut bad = vec![Cell::unknown(); CELL_COUNT];
        bad[500] = Cell {
            height_mm: 1,
            occupancy_bp: 0,
            known: false,
        };
        assert!(SurfaceChunk::from_cells(id, SCENE_FRAME, &bad).is_err());
        bad[500] = Cell::unknown();
        bad[3] = Cell {
            height_mm: 0,
            occupancy_bp: 10_001,
            known: true,
        };
        assert!(SurfaceChunk::from_cells(id, SCENE_FRAME, &bad).is_err());
        assert!(SurfaceChunk::from_cells(id, "lidar", &vec![Cell::unknown(); CELL_COUNT]).is_err());
        assert!(ChunkId::new(100_000, 0).is_err());
    }

    #[test]
    fn observed_cells_keep_height_and_occupancy_apart_from_unknown() {
        let empty = Cell::observed(1_500, 0).unwrap();
        let occupied = Cell::observed(-200, 8_000).unwrap();
        assert!(empty.is_known() && empty.occupancy_bp() == 0);
        assert_eq!(occupied.height_mm(), -200);
        assert!(!Cell::unknown().is_known());
        assert!(Cell::observed(i32::MAX, 0).is_err());
        let mut cells = vec![Cell::unknown(); CELL_COUNT];
        cells[1] = empty;
        let chunk =
            SurfaceChunk::from_cells(ChunkId::new(-1, 0).unwrap(), SCENE_FRAME, &cells).unwrap();
        assert_eq!(chunk.cell(1, 0), Some(&empty));
        assert_eq!(chunk.cell(0, 0).map(|cell| cell.is_known()), Some(false));
        assert_eq!(chunk.cell(32, 0), None);
        assert_eq!(chunk.frame(), SCENE_FRAME);
    }
}
