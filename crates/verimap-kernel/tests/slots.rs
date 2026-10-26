//! Tests for the authority sequence step and the single-slot revision rules.
//! The Lean proofs are about the Aeneas model; these exercise the compiled
//! Rust, including on the wasm32 build's code paths (same source).

#![allow(clippy::panic)] // Test-only crate: helpers panic on unexpected results.

use proptest::prelude::*;
use verimap_kernel::{
    Decision, KernelError, SEQUENCE_MAX, Slot, base_revision_matches, next_sequence, slot_delete,
    slot_upsert,
};

/// Comparable views of the kernel's types, which derive no `PartialEq`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum S {
    Vacant,
    Live(u64),
    Deleted(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum D {
    Apply,
    Duplicate,
    Stale,
    Conflict,
    Tombstoned,
    Missing,
}

fn to_kernel(s: S) -> Slot {
    match s {
        S::Vacant => Slot::Vacant,
        S::Live(revision) => Slot::Live { revision },
        S::Deleted(revision) => Slot::Deleted { revision },
    }
}

fn from_kernel(s: Slot) -> S {
    match s {
        Slot::Vacant => S::Vacant,
        Slot::Live { revision } => S::Live(revision),
        Slot::Deleted { revision } => S::Deleted(revision),
    }
}

fn decision(d: Decision) -> D {
    match d {
        Decision::Apply => D::Apply,
        Decision::Duplicate => D::Duplicate,
        Decision::Stale => D::Stale,
        Decision::Conflict => D::Conflict,
        Decision::Tombstoned => D::Tombstoned,
        Decision::Missing => D::Missing,
    }
}

fn upsert(s: S, revision: u64, same: bool) -> (D, S) {
    let t = slot_upsert(to_kernel(s), revision, same);
    (decision(t.decision), from_kernel(t.slot))
}

fn delete(s: S, revision: u64) -> (D, S) {
    let t = slot_delete(to_kernel(s), revision);
    (decision(t.decision), from_kernel(t.slot))
}

fn revision_of(s: S) -> u64 {
    match s {
        S::Vacant => 0,
        S::Live(r) | S::Deleted(r) => r,
    }
}

fn slot() -> impl Strategy<Value = S> {
    prop_oneof![
        Just(S::Vacant),
        any::<u64>().prop_map(S::Live),
        any::<u64>().prop_map(S::Deleted),
    ]
}

#[test]
fn sequence_steps_by_one_and_never_wraps() {
    assert!(matches!(next_sequence(0), Ok(1)));
    assert!(matches!(next_sequence(41), Ok(42)));
    assert!(matches!(next_sequence(SEQUENCE_MAX - 1), Ok(SEQUENCE_MAX)));
    assert!(matches!(
        next_sequence(SEQUENCE_MAX),
        Err(KernelError::SequenceExhausted)
    ));
}

#[test]
fn upsert_table() {
    use D::*;
    assert_eq!(upsert(S::Vacant, 1, false), (Apply, S::Live(1)));
    assert_eq!(upsert(S::Live(5), 6, false), (Apply, S::Live(6)));
    assert_eq!(upsert(S::Live(5), 5, true), (Duplicate, S::Live(5)));
    assert_eq!(upsert(S::Live(5), 5, false), (Conflict, S::Live(5)));
    assert_eq!(upsert(S::Live(5), 4, true), (Stale, S::Live(5)));
    assert_eq!(upsert(S::Deleted(5), 4, false), (Stale, S::Deleted(5)));
    assert_eq!(upsert(S::Deleted(5), 5, true), (Conflict, S::Deleted(5)));
    assert_eq!(upsert(S::Deleted(5), 6, true), (Tombstoned, S::Deleted(5)));
    assert_eq!(
        upsert(S::Live(SEQUENCE_MAX - 1), SEQUENCE_MAX, false),
        (Apply, S::Live(SEQUENCE_MAX))
    );
}

#[test]
fn delete_table() {
    use D::*;
    assert_eq!(delete(S::Vacant, 3), (Missing, S::Vacant));
    assert_eq!(delete(S::Live(5), 6), (Apply, S::Deleted(6)));
    assert_eq!(delete(S::Live(5), 5), (Conflict, S::Live(5)));
    assert_eq!(delete(S::Live(5), 4), (Stale, S::Live(5)));
    assert_eq!(delete(S::Deleted(5), 5), (Duplicate, S::Deleted(5)));
    assert_eq!(delete(S::Deleted(5), 4), (Stale, S::Deleted(5)));
    assert_eq!(delete(S::Deleted(5), 9), (Tombstoned, S::Deleted(5)));
}

#[test]
fn base_revision_requires_live_at_that_revision() {
    assert!(base_revision_matches(Slot::Live { revision: 7 }, 7));
    assert!(!base_revision_matches(Slot::Live { revision: 7 }, 6));
    assert!(!base_revision_matches(Slot::Live { revision: 7 }, 8));
    assert!(!base_revision_matches(Slot::Deleted { revision: 7 }, 7));
    assert!(!base_revision_matches(Slot::Vacant, 0));
}

proptest! {
    #[test]
    fn only_apply_changes_the_slot(s in slot(), r in any::<u64>(), same in any::<bool>()) {
        for (d, after, applied) in [
            { let (d, a) = upsert(s, r, same); (d, a, S::Live(r)) },
            { let (d, a) = delete(s, r); (d, a, S::Deleted(r)) },
        ] {
            if d == D::Apply {
                prop_assert_eq!(after, applied);
            } else {
                prop_assert_eq!(after, s);
            }
            prop_assert!(revision_of(after) >= revision_of(s));
        }
    }

    #[test]
    fn tombstones_are_permanent(d in any::<u64>(), r in any::<u64>(), same in any::<bool>()) {
        prop_assert_eq!(upsert(S::Deleted(d), r, same).1, S::Deleted(d));
        prop_assert_eq!(delete(S::Deleted(d), r).1, S::Deleted(d));
        prop_assert_ne!(upsert(S::Deleted(d), r, same).0, D::Apply);
    }

    #[test]
    fn applying_twice_is_applying_once(s in slot(), r in any::<u64>()) {
        let (d1, once) = upsert(s, r, true);
        if d1 == D::Apply {
            prop_assert_eq!(upsert(once, r, true), (D::Duplicate, once));
        }
        let (d1, once) = delete(s, r);
        if d1 == D::Apply {
            prop_assert_eq!(delete(once, r), (D::Duplicate, once));
        }
    }
}
