#![allow(clippy::expect_used)]
use super::*;
#[test]
fn full_incremental_and_restore_chains_are_complete() {
    let mut catalog = Catalog::<2>::new();
    assert!(catalog.begin(Some(SnapshotId(1))).is_none());
    let full = catalog.begin(None).expect("full");
    let full = catalog.commit(full, 11).expect("commit");
    let delta = catalog.begin(Some(full.id)).expect("delta");
    let delta = catalog.commit(delta, 22).expect("commit");
    assert_eq!(
        catalog.restore_plan(delta.id),
        Some([Some(full), Some(delta)])
    );
    assert!(catalog.begin(Some(delta.id)).is_none());
    let replacement = catalog.begin(None).expect("full replacement");
    assert_eq!(catalog.latest(), Some(delta));
    let replacement = catalog.commit(replacement, 33).expect("commit");
    assert_eq!(catalog.restore_plan(full.id), None);
    assert_eq!(
        catalog.restore_plan(replacement.id),
        Some([Some(replacement), None])
    );
}
#[test]
fn aborts_do_not_erase_old_checkpoints_or_allow_stale_commits() {
    let mut catalog = Catalog::<1>::new();
    let old = catalog.begin(None).expect("attempt");
    assert!(catalog.abort(old));
    let new = catalog.begin(None).expect("replacement");
    assert!(new.id() > old.id());
    assert_eq!(catalog.commit(old, 0), None);
    assert!(catalog.commit(new, 1).is_some());
    assert!(Catalog::<0>::new().begin(None).is_none());
    catalog.sequence = u64::MAX;
    assert!(catalog.begin(None).is_none());
}
