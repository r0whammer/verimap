//! Determinism of the demo session: tick batching, journal replay, and the
//! committed fixture `fixtures/replay/demo-seed-1.json`, which the browser
//! end-to-end test also checks against the WASM build.
//!
//! Regenerate the fixture after an intentional behavior change (and bump
//! `SIMULATOR_VERSION` or `SCENE_VERSION`) with:
//! `VERIMAP_BLESS=1 cargo test -p verimap-sim --test replay`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use serde_json::{Value, json};
use verimap_domain::{Freshness, replay};
use verimap_sim::{DEMO_SEED, LocalSession, SCENE_VERSION, SIMULATOR_VERSION};

const CHECKPOINT_TICKS: [u64; 4] = [100, 300, 600, 1600];

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/replay/demo-seed-1.json")
}

fn run_in_chunks(ticks: u64, chunk: u32) -> LocalSession {
    let mut s = LocalSession::new(DEMO_SEED).unwrap();
    let mut done = 0;
    while done < ticks {
        let n = u32::try_from((ticks - done).min(u64::from(chunk))).unwrap();
        s.advance(n).ok().unwrap();
        done += u64::from(n);
    }
    s
}

#[test]
fn journal_does_not_depend_on_tick_batching() {
    let one = run_in_chunks(500, 1);
    let seven = run_in_chunks(500, 7);
    let all = run_in_chunks(500, 500);
    assert_eq!(one.authority().journal(), seven.authority().journal());
    assert_eq!(one.authority().journal(), all.authority().journal());
    assert_eq!(one.authority().store(), all.authority().store());
    assert_eq!(one.stats(), all.stats());
}

#[test]
fn replay_matches_live_and_the_committed_fixture() {
    let mut session = LocalSession::new(DEMO_SEED).unwrap();
    let mut checkpoints = Vec::new();
    for &tick in &CHECKPOINT_TICKS {
        let now = session.authority().store().tick();
        session
            .advance(u32::try_from(tick - now).unwrap())
            .ok()
            .unwrap();
        let store = session.authority().store();
        assert_eq!(store.tick(), tick);
        checkpoints.push(json!({
            "tick": tick,
            "last_sequence": store.last_sequence(),
            "state_hash": format!("{:016x}", store.state_hash()),
        }));
    }
    let authority = session.authority();
    let site = authority.store().site().clone();
    assert_eq!(
        &replay(site, LocalSession::EPOCH, authority.journal()).unwrap(),
        authority.store()
    );
    for c in &checkpoints {
        let seq = c["last_sequence"].as_u64().unwrap();
        let hash = format!("{:016x}", authority.replay_to(seq).unwrap().state_hash());
        assert_eq!(c["state_hash"], hash.as_str(), "replay to {seq}");
    }

    let actual = json!({
        "description": "Native state hashes of the demo session; see crates/verimap-sim/tests/replay.rs",
        "seed": DEMO_SEED,
        "simulator_version": SIMULATOR_VERSION,
        "scene_version": SCENE_VERSION,
        "checkpoints": checkpoints,
    });
    let path = fixture_path();
    if std::env::var_os("VERIMAP_BLESS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut text = serde_json::to_string_pretty(&actual).unwrap();
        text.push('\n');
        std::fs::write(&path, text).unwrap();
    }
    let expected: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("fixture exists")).unwrap();
    assert_eq!(
        actual,
        expected,
        "demo session differs from {}; if intended, bump the version and re-bless",
        path.display()
    );
}

#[test]
fn demo_vehicle_goes_stale_behind_the_building_and_keeps_its_track() {
    let mut session = LocalSession::new(DEMO_SEED).unwrap();
    let mut seen = Vec::new();
    for _ in 0..1600 {
        session.advance(1).ok().unwrap();
        let store = session.authority().store();
        for (_, e) in store.entities() {
            let f = e.freshness(store.tick());
            if seen.last() != Some(&f) {
                seen.push(f);
            }
        }
    }
    let store = session.authority().store();
    let ids: Vec<_> = store
        .entities()
        .map(|(_, e)| e.id().as_str().to_owned())
        .collect();
    assert_eq!(ids, ["trk:1"], "one vehicle, one track across two laps");
    assert_eq!(
        seen,
        [
            Freshness::Active,
            Freshness::Stale,
            Freshness::Active,
            Freshness::Stale,
            Freshness::Active
        ]
    );
    let stats = session.stats();
    assert_eq!(
        stats.received,
        stats.accepted + stats.rejected + stats.duplicates + stats.late
    );
    assert!(stats.late > 0, "delivery delay produces late observations");
    assert_eq!(stats.rejected, 0);
}

#[test]
fn a_full_journal_stops_the_session_without_losing_progress() {
    let mut session = LocalSession::with_journal_limit(DEMO_SEED, 50).unwrap();
    let advanced = session.advance(100);
    assert_eq!(
        advanced.stopped,
        Some(verimap_domain::CommandError::JournalFull { limit: 50 })
    );
    assert!(advanced.changes.clock);
    let store = session.authority().store();
    assert_eq!(session.authority().journal().len(), 50);
    assert_eq!(store.last_sequence(), 50);
    // Stopped, not skipped: the next advance fails before simulating.
    let tick = store.tick();
    assert!(session.advance(1).stopped.is_some());
    assert_eq!(session.authority().store().tick(), tick);
}

#[test]
fn a_removed_track_stays_deleted_and_later_detections_start_a_new_one() {
    let mut session = run_in_chunks(100, 100);
    let trk1 = verimap_domain::ObjectId::parse("id", "trk:1").unwrap();
    let (revision, _) = session.authority().store().entity(&trk1).unwrap();
    session
        .submit(verimap_domain::Command::RemoveEntity {
            id: trk1.clone(),
            base_revision: revision,
        })
        .unwrap();
    session.advance(20).ok().unwrap();
    let store = session.authority().store();
    assert!(store.entity(&trk1).is_none());
    assert!(matches!(
        store.entity_slot(&trk1),
        verimap_domain::Slot::Deleted { .. }
    ));
    let ids: Vec<_> = store
        .entities()
        .map(|(_, e)| e.id().as_str().to_owned())
        .collect();
    assert_eq!(
        ids,
        ["trk:2"],
        "the vehicle is tracked again under a new ID"
    );
}
