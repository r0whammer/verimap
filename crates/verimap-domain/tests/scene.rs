//! Behavior of the scene store, authority, journal replay, and tracker.

// Test-only crate: helpers unwrap known-valid values and panic on surprises.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use proptest::prelude::*;
use verimap_domain::{
    ApplyError, Association, Authority, Command, CommandError, CommittedEvent, Confidence,
    Detection, EntityClass, EventPayload, Freshness, GeodeticAnchor, ObjectId, ObservationRef,
    Placement, Provenance, Refusal, STALE_AFTER_TICKS, SceneBounds, ScenePoint, SiteFrame, SiteId,
    SourceId, TrackFix, TrackTarget, TrackerConfig, Uncertainty, associate, replay,
};

fn site() -> SiteFrame {
    SiteFrame::new(
        SiteId::new("test-site").unwrap(),
        "Test",
        GeodeticAnchor::new(1.0, 2.0, 0.0).unwrap(),
        SceneBounds::new((-100_000, 100_000), (-100_000, 100_000), (-20_000, 80_000)).unwrap(),
    )
    .unwrap()
}

fn authority() -> Authority {
    Authority::new(site(), 1, 10_000)
}

fn id(s: &str) -> ObjectId {
    ObjectId::parse("id", s).unwrap()
}

fn point(x_m: i64, y_m: i64) -> ScenePoint {
    ScenePoint::new(x_m * 1000, y_m * 1000, 0).unwrap()
}

fn fix(tick: u64, x_m: i64, y_m: i64) -> TrackFix {
    TrackFix {
        acquired_tick: tick,
        position: point(x_m, y_m),
        uncertainty: Uncertainty::new(1_500).unwrap(),
        confidence: Confidence::new(9_000).unwrap(),
    }
}

fn source(sequence: u64) -> ObservationRef {
    ObservationRef {
        source: SourceId::new("s", "cam-1").unwrap(),
        sequence,
    }
}

fn observe(target: TrackTarget, tick: u64, x_m: i64, y_m: i64) -> Command {
    Command::ObserveEntity {
        target,
        class: EntityClass::Vehicle,
        fix: fix(tick, x_m, y_m),
        source: source(tick),
    }
}

fn advance(a: &mut Authority, to_tick: u64) {
    a.submit(Command::AdvanceClock { to_tick }).unwrap();
}

fn add_marker(a: &mut Authority, label: &str) -> ObjectId {
    let before = a.store().annotation_record_count();
    a.submit(Command::AddMarker {
        label: label.into(),
        position: point(1, 2),
        placement: Placement::GroundPlane,
    })
    .unwrap();
    assert_eq!(a.store().annotation_record_count(), before + 1);
    let Some(CommittedEvent {
        payload: EventPayload::AnnotationPut { annotation },
        ..
    }) = a.journal().last()
    else {
        panic!("expected an annotation event");
    };
    annotation.id().clone()
}

/// Every failed submit leaves the store and journal exactly as they were.
fn assert_rejected(a: &mut Authority, command: Command) -> CommandError {
    let (store, journal) = (a.store().clone(), a.journal().to_vec());
    let err = a.submit(command).expect_err("command should be rejected");
    assert_eq!(a.store(), &store);
    assert_eq!(a.journal(), journal.as_slice());
    err
}

#[test]
fn sequences_increase_by_one_and_revisions_follow_them() {
    let mut a = authority();
    advance(&mut a, 1);
    let m = add_marker(&mut a, "Gate");
    advance(&mut a, 2);
    let sequences: Vec<u64> = a.journal().iter().map(|e| e.sequence).collect();
    assert_eq!(sequences, [1, 2, 3]);
    assert_eq!(a.store().annotation(&m).map(|(r, _)| r), Some(2));
    assert_eq!(a.store().last_sequence(), 3);
    assert_eq!(a.store().tick(), 2);
}

#[test]
fn store_rejects_out_of_order_events_without_change() {
    let mut a = authority();
    advance(&mut a, 1);
    advance(&mut a, 2);
    let journal = a.journal().to_vec();
    let mut store = replay(site(), 1, &journal[..1]).unwrap();
    let before = store.clone();
    let mut wrong_epoch = journal[1].clone();
    wrong_epoch.epoch = 2;
    assert_eq!(
        store.apply(&wrong_epoch),
        Err(ApplyError::EpochMismatch {
            expected: 1,
            found: 2
        })
    );
    assert_eq!(
        store.apply(&journal[0]),
        Err(ApplyError::AlreadyApplied { sequence: 1 })
    );
    let mut gap = journal[1].clone();
    gap.sequence = 3;
    assert_eq!(
        store.apply(&gap),
        Err(ApplyError::SequenceGap {
            expected: 2,
            found: 3
        })
    );
    let mut wrong_provenance = journal[1].clone();
    wrong_provenance.provenance = Provenance::Operator;
    assert_eq!(
        store.apply(&wrong_provenance),
        Err(ApplyError::ProvenanceMismatch)
    );
    assert_eq!(store, before);
    assert!(store.apply(&journal[1]).is_ok());
}

#[test]
fn clock_only_moves_forward() {
    let mut a = authority();
    advance(&mut a, 5);
    for to_tick in [5, 4, 0] {
        assert_eq!(
            assert_rejected(&mut a, Command::AdvanceClock { to_tick }),
            CommandError::Rejected(ApplyError::ClockNotAdvanced { tick: 5 })
        );
    }
}

#[test]
fn moving_an_entity_keeps_its_identity() {
    let mut a = authority();
    advance(&mut a, 1);
    a.submit(observe(TrackTarget::New, 1, 0, 0)).unwrap();
    advance(&mut a, 2);
    a.submit(observe(TrackTarget::Existing(id("trk:1")), 2, 30, -40))
        .unwrap();
    let ids: Vec<_> = a.store().entities().map(|(_, e)| e.id().clone()).collect();
    assert_eq!(ids, [id("trk:1")]);
    let (revision, e) = a.store().entity(&id("trk:1")).unwrap();
    assert_eq!(revision, 4);
    assert_eq!(e.latest().position, point(30, -40));
    assert_eq!(e.history().len(), 2);
}

#[test]
fn observations_cannot_come_from_the_future_or_overwrite_newer_fixes() {
    let mut a = authority();
    advance(&mut a, 10);
    assert_eq!(
        assert_rejected(&mut a, observe(TrackTarget::New, 11, 0, 0)),
        CommandError::Rejected(ApplyError::ObservedInFuture { id: id("trk:1") })
    );
    a.submit(observe(TrackTarget::New, 8, 0, 0)).unwrap();
    // A late fix (acquired 7 < latest 8) is refused; the same tick is fine.
    assert_eq!(
        assert_rejected(&mut a, observe(TrackTarget::Existing(id("trk:1")), 7, 1, 1)),
        CommandError::Rejected(ApplyError::LateObservation { id: id("trk:1") })
    );
    a.submit(observe(TrackTarget::Existing(id("trk:1")), 8, 1, 1))
        .unwrap();
}

#[test]
fn track_class_is_fixed_and_positions_stay_inside_the_site() {
    let mut a = authority();
    advance(&mut a, 1);
    a.submit(observe(TrackTarget::New, 1, 0, 0)).unwrap();
    let mut person = observe(TrackTarget::Existing(id("trk:1")), 1, 0, 0);
    if let Command::ObserveEntity { class, .. } = &mut person {
        *class = EntityClass::Person;
    }
    assert_eq!(
        assert_rejected(&mut a, person),
        CommandError::Rejected(ApplyError::ClassChanged { id: id("trk:1") })
    );
    assert_eq!(
        assert_rejected(&mut a, observe(TrackTarget::New, 1, 101, 0)),
        CommandError::Rejected(ApplyError::OutsideSite { id: id("trk:2") })
    );
}

#[test]
fn deleted_tracks_stay_deleted_and_ids_are_not_reused() {
    let mut a = authority();
    advance(&mut a, 1);
    a.submit(observe(TrackTarget::New, 1, 0, 0)).unwrap();
    let (revision, _) = a.store().entity(&id("trk:1")).unwrap();
    a.submit(Command::RemoveEntity {
        id: id("trk:1"),
        base_revision: revision,
    })
    .unwrap();
    assert!(a.store().entity(&id("trk:1")).is_none());
    assert_eq!(
        assert_rejected(&mut a, observe(TrackTarget::Existing(id("trk:1")), 1, 0, 0)),
        CommandError::Rejected(ApplyError::Refused {
            id: id("trk:1"),
            refusal: Refusal::Tombstoned
        })
    );
    assert_eq!(
        assert_rejected(
            &mut a,
            Command::RemoveEntity {
                id: id("trk:1"),
                base_revision: revision
            }
        ),
        CommandError::Deleted { id: id("trk:1") }
    );
    a.submit(observe(TrackTarget::New, 1, 0, 0)).unwrap();
    assert!(a.store().entity(&id("trk:2")).is_some());
}

#[test]
fn edits_require_the_current_revision() {
    let mut a = authority();
    let m = add_marker(&mut a, "Gate");
    let (rev1, _) = a.store().annotation(&m).unwrap();
    a.submit(Command::EditMarker {
        id: m.clone(),
        base_revision: rev1,
        label: Some("North gate".into()),
        position: None,
    })
    .unwrap();
    let (rev2, marker) = a.store().annotation(&m).unwrap();
    assert!(rev2 > rev1);
    assert_eq!(marker.label(), "North gate");
    assert_eq!(marker.position(), &point(1, 2));
    // An edit based on the old revision does not overwrite the newer one.
    assert_eq!(
        assert_rejected(
            &mut a,
            Command::EditMarker {
                id: m.clone(),
                base_revision: rev1,
                label: Some("Old".into()),
                position: None
            }
        ),
        CommandError::PreconditionFailed {
            id: m.clone(),
            base_revision: rev1,
            current: rev2
        }
    );
    assert_eq!(
        assert_rejected(
            &mut a,
            Command::EditMarker {
                id: m.clone(),
                base_revision: rev2,
                label: Some("North gate".into()),
                position: None
            }
        ),
        CommandError::NothingToChange
    );
    assert!(matches!(
        assert_rejected(
            &mut a,
            Command::EditMarker {
                id: m.clone(),
                base_revision: rev2,
                label: Some("bad\nlabel".into()),
                position: None
            }
        ),
        CommandError::Invalid(_)
    ));
    a.submit(Command::DeleteAnnotation {
        id: m.clone(),
        base_revision: rev2,
    })
    .unwrap();
    assert_eq!(
        assert_rejected(
            &mut a,
            Command::EditMarker {
                id: m.clone(),
                base_revision: rev2,
                label: Some("Back".into()),
                position: None
            }
        ),
        CommandError::Deleted { id: m.clone() }
    );
    assert_eq!(
        assert_rejected(
            &mut a,
            Command::DeleteAnnotation {
                id: id("op:9"),
                base_revision: 1
            }
        ),
        CommandError::NotFound { id: id("op:9") }
    );
    assert_eq!(add_marker(&mut a, "Again"), id("op:2"));
}

#[test]
fn markers_outside_the_site_are_rejected() {
    let mut a = authority();
    assert_eq!(
        assert_rejected(
            &mut a,
            Command::AddMarker {
                label: "Far".into(),
                position: point(0, 150),
                placement: Placement::GroundPlane,
            }
        ),
        CommandError::Rejected(ApplyError::OutsideSite { id: id("op:1") })
    );
}

#[test]
fn clock_reports_freshness_changes() {
    let mut a = authority();
    advance(&mut a, 1);
    a.submit(observe(TrackTarget::New, 1, 0, 0)).unwrap();
    let changes = a
        .submit(Command::AdvanceClock {
            to_tick: 1 + STALE_AFTER_TICKS,
        })
        .unwrap();
    assert!(changes.clock && changes.entities.is_empty());
    let changes = a
        .submit(Command::AdvanceClock {
            to_tick: 2 + STALE_AFTER_TICKS,
        })
        .unwrap();
    assert_eq!(
        changes.entities.into_iter().collect::<Vec<_>>(),
        [id("trk:1")]
    );
    let (_, e) = a.store().entity(&id("trk:1")).unwrap();
    assert_eq!(e.freshness(a.store().tick()), Freshness::Stale);
}

#[test]
fn journal_limit_stops_ingestion_with_an_error() {
    let mut a = Authority::new(site(), 1, 3);
    for t in 1..=3 {
        advance(&mut a, t);
    }
    assert_eq!(
        assert_rejected(&mut a, Command::AdvanceClock { to_tick: 4 }),
        CommandError::JournalFull { limit: 3 }
    );
}

#[test]
fn replay_rebuilds_every_prefix_exactly() {
    let mut a = authority();
    let mut snapshots = vec![a.store().clone()];
    let m = add_marker(&mut a, "Gate");
    snapshots.push(a.store().clone());
    for t in 1..=40 {
        advance(&mut a, t);
        snapshots.push(a.store().clone());
        let target = if t == 1 {
            TrackTarget::New
        } else {
            TrackTarget::Existing(id("trk:1"))
        };
        let x = i64::try_from(t).unwrap();
        a.submit(observe(target, t, x, -x)).unwrap();
        snapshots.push(a.store().clone());
    }
    let (rev, _) = a.store().annotation(&m).unwrap();
    a.submit(Command::DeleteAnnotation {
        id: m,
        base_revision: rev,
    })
    .unwrap();
    snapshots.push(a.store().clone());

    let full = replay(site(), 1, a.journal()).unwrap();
    assert_eq!(&full, a.store());
    assert_eq!(full.state_hash(), a.store().state_hash());
    for (k, snapshot) in snapshots.iter().enumerate() {
        let replayed = a.replay_to(u64::try_from(k).unwrap()).unwrap();
        assert_eq!(&replayed, snapshot, "prefix {k}");
        assert_eq!(replayed.state_hash(), snapshot.state_hash());
    }
    // Replaying never touches the live state.
    assert_eq!(a.store(), &full);
}

#[test]
fn state_hash_distinguishes_states() {
    let mut a = authority();
    let empty = a.store().state_hash();
    add_marker(&mut a, "A");
    let one = a.store().state_hash();
    assert_ne!(empty, one);
    let mut b = authority();
    add_marker(&mut b, "B");
    assert_ne!(one, b.store().state_hash());
}

// --- Tracker ---------------------------------------------------------------

fn detection(class: EntityClass, tick: u64, x_m: i64, y_m: i64) -> Detection {
    Detection {
        source: source(tick),
        class,
        fix: fix(tick, x_m, y_m),
    }
}

/// A store with vehicle tracks at the given (x, y) metres, observed at tick 10.
fn with_tracks(positions: &[(i64, i64)]) -> Authority {
    let mut a = authority();
    advance(&mut a, 10);
    for &(x, y) in positions {
        a.submit(observe(TrackTarget::New, 10, x, y)).unwrap();
    }
    advance(&mut a, 20);
    a
}

#[test]
fn associates_nearest_same_class_track_within_gates() {
    let config = TrackerConfig::default();
    let a = with_tracks(&[(0, 0), (40, 0)]);
    let store = a.store();
    assert_eq!(
        associate(&config, store, &detection(EntityClass::Vehicle, 11, 3, 0)),
        Association::Existing(id("trk:1"))
    );
    assert_eq!(
        associate(&config, store, &detection(EntityClass::Vehicle, 11, 37, 0)),
        Association::Existing(id("trk:2"))
    );
    // Wrong class: no candidate.
    assert_eq!(
        associate(&config, store, &detection(EntityClass::Person, 11, 0, 0)),
        Association::New
    );
    // Outside the distance gate (5 m + 1 m per tick, here 1 tick: 6 m).
    assert_eq!(
        associate(&config, store, &detection(EntityClass::Vehicle, 11, 20, 0)),
        Association::New
    );
    // The gate grows with elapsed time: 10 ticks later 15 m is inside.
    assert_eq!(
        associate(&config, store, &detection(EntityClass::Vehicle, 20, 15, 0)),
        Association::Existing(id("trk:1"))
    );
}

#[test]
fn equal_distances_go_to_the_smallest_id() {
    let a = with_tracks(&[(-2, 0), (2, 0)]);
    assert_eq!(
        associate(
            &TrackerConfig::default(),
            a.store(),
            &detection(EntityClass::Vehicle, 11, 0, 0)
        ),
        Association::Existing(id("trk:1"))
    );
}

#[test]
fn late_detection_is_not_applied() {
    let a = with_tracks(&[(0, 0)]);
    assert_eq!(
        associate(
            &TrackerConfig::default(),
            a.store(),
            &detection(EntityClass::Vehicle, 9, 0, 0)
        ),
        Association::Late(id("trk:1"))
    );
}

#[test]
fn tracks_unseen_beyond_the_coast_limit_are_not_candidates() {
    let config = TrackerConfig::default();
    let a = with_tracks(&[(0, 0)]);
    let t = 10 + config.max_coast_ticks;
    assert_eq!(
        associate(
            &config,
            a.store(),
            &detection(EntityClass::Vehicle, t, 0, 0)
        ),
        Association::Existing(id("trk:1"))
    );
    assert_eq!(
        associate(
            &config,
            a.store(),
            &detection(EntityClass::Vehicle, t + 1, 0, 0)
        ),
        Association::New
    );
}

#[test]
fn crossing_tracks_follow_the_documented_rule() {
    // Two vehicles approach each other along x and cross at the origin.
    // Nearest-neighbour association is deterministic but can swap IDs at
    // the crossing; this test pins the rule's behavior rather than claiming
    // identity is preserved.
    let config = TrackerConfig::default();
    let mut a = with_tracks(&[(-10, 0), (10, 0)]);
    let mut assigned = Vec::new();
    for (step, x) in [(21_u64, -5_i64), (22, 0), (23, 5)] {
        advance(&mut a, step);
        let mut ids = Vec::new();
        for x in [x, -x] {
            let d = detection(EntityClass::Vehicle, step, x, 0);
            let Association::Existing(track) = associate(&config, a.store(), &d) else {
                panic!("detections stay inside the gate");
            };
            a.submit(Command::ObserveEntity {
                target: TrackTarget::Existing(track.clone()),
                class: d.class,
                fix: d.fix,
                source: source(step * 2 + u64::from(x > 0)),
            })
            .unwrap();
            ids.push(track);
        }
        assigned.push(ids);
    }
    assert_eq!(assigned[0], [id("trk:1"), id("trk:2")]);
    // At the crossing both detections are at x = 0. The first goes to
    // trk:1 by the ID tie-break; the second also goes to trk:1, which now
    // sits at x = 0. The baseline has no one-detection-per-track constraint
    // within a frame, so trk:2 receives nothing and coasts.
    assert_eq!(assigned[1], [id("trk:1"), id("trk:1")]);
    assert_eq!(assigned[2].len(), 2);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Arbitrary command sequences, valid or not: the journal replays to
    /// the live state, and failed commands change nothing.
    #[test]
    fn journal_replay_matches_live_state(ops in prop::collection::vec((0u8..6, 0u64..4, -120i64..120, -120i64..120), 1..60)) {
        let mut a = authority();
        for (op, n, x, y) in ops {
            let tick = a.store().tick();
            let command = match op {
                0 => Command::AdvanceClock { to_tick: tick + n },
                1 => observe(TrackTarget::New, tick.saturating_sub(n), x, y),
                2 => observe(TrackTarget::Existing(ObjectId::from_parts("id", "trk", &(n + 1).to_string()).unwrap()), tick.saturating_sub(n), x, y),
                3 => Command::AddMarker { label: format!("m{n}"), position: ScenePoint::new(x * 1000, y * 1000, 0).unwrap(), placement: Placement::GroundPlane },
                4 => Command::DeleteAnnotation { id: ObjectId::from_parts("id", "op", &(n + 1).to_string()).unwrap(), base_revision: a.store().annotation(&ObjectId::from_parts("id", "op", &(n + 1).to_string()).unwrap()).map_or(0, |(r, _)| r) },
                _ => Command::RemoveEntity { id: ObjectId::from_parts("id", "trk", &(n + 1).to_string()).unwrap(), base_revision: n },
            };
            let before = a.store().clone();
            let journal_len = a.journal().len();
            match a.submit(command) {
                Ok(_) => prop_assert_eq!(a.journal().len(), journal_len + 1),
                Err(_) => {
                    prop_assert_eq!(a.store(), &before);
                    prop_assert_eq!(a.journal().len(), journal_len);
                }
            }
        }
        let replayed = replay(site(), 1, a.journal()).unwrap();
        prop_assert_eq!(&replayed, a.store());
        // Revisions never exceed the last sequence, and sequences are 1..=n.
        for (i, e) in a.journal().iter().enumerate() {
            prop_assert_eq!(e.sequence, u64::try_from(i).unwrap() + 1);
        }
        for (r, _) in a.store().entities() { prop_assert!(r <= a.store().last_sequence()); }
    }
}
