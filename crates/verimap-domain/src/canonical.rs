//! Canonical encoding of scene state, for comparing replays exactly.
//!
//! The encoding is written field by field in a fixed order. Maps are
//! `BTreeMap`s, so objects appear in ID byte order, never in hash order.
//! Integers are little-endian and strings are length-prefixed UTF-8. State
//! holds only integers and strings, so native and WASM builds produce the
//! same bytes. The hash is 64-bit FNV-1a, a fast checksum for equality
//! tests and diagnostics, not a cryptographic digest.

use crate::store::{Record, Records};
use crate::{Annotation, Entity, SceneStore, TrackFix};

/// Identifies the encoding; bump it whenever the layout changes.
const MAGIC: &[u8] = b"verimap-scene-state-v1";

struct Encoder {
    out: Vec<u8>,
}

impl Encoder {
    fn u8(&mut self, v: u8) {
        self.out.push(v);
    }

    fn u16(&mut self, v: u16) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    fn u32(&mut self, v: u32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    fn u64(&mut self, v: u64) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    fn i64(&mut self, v: i64) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    fn len(&mut self, n: usize) {
        // usize is at most 64 bits on every supported target.
        self.u64(u64::try_from(n).unwrap_or(u64::MAX));
    }

    fn str(&mut self, s: &str) {
        self.len(s.len());
        self.out.extend_from_slice(s.as_bytes());
    }

    fn fix(&mut self, f: &TrackFix) {
        self.u64(f.acquired_tick);
        self.i64(f.position.x_mm());
        self.i64(f.position.y_mm());
        self.i64(f.position.z_mm());
        self.u32(f.uncertainty.sigma_mm());
        self.u16(f.confidence.basis_points());
    }

    fn records<T>(&mut self, records: &Records<T>, mut value: impl FnMut(&mut Self, &T)) {
        self.len(records.len());
        for (id, record) in records {
            self.str(id.as_str());
            match record {
                Record::Live { revision, value: v } => {
                    self.u8(1);
                    self.u64(*revision);
                    value(self, v);
                }
                Record::Deleted { revision } => {
                    self.u8(2);
                    self.u64(*revision);
                }
            }
        }
    }

    fn entity(&mut self, e: &Entity) {
        self.str(e.class().as_str());
        self.fix(e.latest());
        self.str(e.source().source.as_str());
        self.u64(e.source().sequence);
        self.len(e.history().len());
        for h in e.history() {
            self.fix(&h.fix);
            self.str(h.source.source.as_str());
            self.u64(h.source.sequence);
            self.u64(h.revision);
        }
    }

    fn annotation(&mut self, a: &Annotation) {
        self.str(a.label());
        self.i64(a.position().x_mm());
        self.i64(a.position().y_mm());
        self.i64(a.position().z_mm());
        self.str(a.placement().as_str());
    }
}

impl SceneStore {
    /// The canonical encoding of this state (see the module documentation).
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut e = Encoder { out: Vec::new() };
        e.out.extend_from_slice(MAGIC);
        e.str(self.site().id().as_str());
        e.u32(self.epoch());
        e.u64(self.last_sequence());
        e.u64(self.tick());
        let (entities, annotations) = self.records();
        e.records(entities, Encoder::entity);
        e.records(annotations, Encoder::annotation);
        e.out
    }

    /// 64-bit FNV-1a of [`SceneStore::canonical_bytes`].
    pub fn state_hash(&self) -> u64 {
        fnv1a64(&self.canonical_bytes())
    }
}

/// 64-bit FNV-1a.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    bytes
        .iter()
        .fold(OFFSET, |h, b| (h ^ u64::from(*b)).wrapping_mul(PRIME))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a64_reference_values() {
        // Published FNV-1a test vectors.
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
    }
}
