//! The chain: full, incremental, replacement, capacity, restore plans.

use super::*;

#[test]
fn full_incremental_and_restore_chains_are_complete() {
    let mut catalog = Catalog::<2>::new();
    assert_eq!(catalog.capacity(), 2);
    let full = begun(&mut catalog, None);
    assert_eq!(catalog.pending(), Some(full));
    let full = committed(&mut catalog, full, 11);
    assert_eq!(catalog.pending(), None);
    let delta = begun(&mut catalog, Some(full.id()));
    let delta = committed(&mut catalog, delta, 22);
    assert_eq!(delta.base(), Some(full.id()));
    assert_eq!(plan(&catalog, delta.id()), Ok(std::vec![full, delta]));
    assert_eq!(plan(&catalog, full.id()), Ok(std::vec![full]));
    refused(&mut catalog, Refusal::ChainFull, |c| {
        c.begin(Some(delta.id()))
    });
    let replacement = begun(&mut catalog, None);
    assert_eq!(catalog.latest(), Some(delta));
    assert_eq!(plan(&catalog, delta.id()), Ok(std::vec![full, delta]));
    let replacement = committed(&mut catalog, replacement, 33);
    refused(&mut catalog, Refusal::Stale, |c| plan(c, full.id()));
    assert_eq!(plan(&catalog, replacement.id()), Ok(std::vec![replacement]));
}

#[test]
fn begin_refusals_name_each_reason_and_change_nothing() {
    let mut catalog = Catalog::<2>::new();
    // Never issued: refused before anything else is looked at.
    refused(&mut catalog, Refusal::Stale, |c| c.begin(Some(id(1))));
    let full = begun(&mut catalog, None);
    refused(&mut catalog, Refusal::Busy, |c| c.begin(None));
    refused(&mut catalog, Refusal::Busy, |c| c.begin(Some(full.id())));
    refused(&mut catalog, Refusal::Stale, |c| c.begin(Some(id(9))));
    let full = committed(&mut catalog, full, 1);
    let delta = begun(&mut catalog, Some(full.id()));
    let delta = committed(&mut catalog, delta, 2);
    refused(&mut catalog, Refusal::NotLatest, |c| {
        c.begin(Some(full.id()))
    });
    refused(&mut catalog, Refusal::ChainFull, |c| {
        c.begin(Some(delta.id()))
    });
    let aborted = begun(&mut catalog, None);
    assert_eq!(catalog.abort(aborted), Ok(Aborting::Aborted));
    refused(&mut catalog, Refusal::NotLatest, |c| {
        c.begin(Some(aborted.id()))
    });
    let mut empty = Catalog::<0>::new();
    refused(&mut empty, Refusal::ChainFull, |c| c.begin(None));
}

#[test]
fn an_exhausted_sequence_refuses_begin_without_consuming() {
    let mut catalog = Catalog::<1>::restored(3, periscope_machines::SEQUENCE_MAX - 1, &[])
        .expect("one id short of the end is within the sequence");
    let last = begun(&mut catalog, None);
    assert_eq!(
        last.id().raw() & periscope_machines::SEQUENCE_MAX,
        periscope_machines::SEQUENCE_MAX
    );
    let last = committed(&mut catalog, last, 5);
    refused(&mut catalog, Refusal::Exhausted, |c| c.begin(None));
    // Capacity is judged before the sequence.
    refused(&mut catalog, Refusal::ChainFull, |c| {
        c.begin(Some(last.id()))
    });
    assert_eq!(plan(&catalog, last.id()), Ok(std::vec![last]));
}

#[test]
fn aborts_keep_old_checkpoints_and_never_reuse_ids() {
    let mut catalog = Catalog::<1>::new();
    let old = begun(&mut catalog, None);
    assert_eq!(catalog.abort(old), Ok(Aborting::Aborted));
    let new = begun(&mut catalog, None);
    assert!(
        new.id().raw() > old.id().raw(),
        "an aborted id was reissued"
    );
    refused(&mut catalog, Refusal::Stale, |c| c.commit(old, 0));
    committed(&mut catalog, new, 1);
}

#[test]
fn restore_plan_writes_only_the_plan_and_refuses_a_short_buffer() {
    let mut catalog = Catalog::<4>::new();
    let mut base = None;
    let mut chain = std::vec::Vec::new();
    for checksum in 0..3 {
        let pending = begun(&mut catalog, base);
        let manifest = committed(&mut catalog, pending, checksum);
        base = Some(manifest.id());
        chain.push(manifest);
    }
    let marker = Manifest::new(id(99), None, 99).expect("valid");
    let mut out = [marker; 5];
    assert_eq!(catalog.restore_plan(chain[2].id(), &mut out), Ok(3));
    assert_eq!(&out[..3], &chain[..]);
    assert_eq!(&out[3..], &[marker; 2]);
    let mut short = [marker; 2];
    assert_eq!(
        catalog.restore_plan(chain[2].id(), &mut short),
        Err(Refusal::Undersized)
    );
    assert_eq!(short, [marker; 2], "a refused plan wrote into the buffer");
    assert_eq!(catalog.restore_plan(chain[1].id(), &mut short), Ok(2));
    let open = begun(&mut catalog, base);
    assert_eq!(
        catalog.restore_plan(open.id(), &mut out),
        Err(Refusal::Stale)
    );
}

#[test]
fn values_are_valid_or_do_not_exist() {
    assert_eq!(SnapshotId::from_raw(0), None);
    assert_eq!(
        SnapshotId::from_raw(1 << 48),
        None,
        "sequence zero in scope one"
    );
    assert_eq!(id((7 << 48) | 2).scope(), 7);
    assert_eq!(Pending::new(id(2), Some(id(2))), None);
    assert_eq!(Pending::new(id(2), Some(id(3))), None);
    assert_eq!(
        Pending::new(id(2), Some(id((1 << 48) | 1))),
        None,
        "base of another scope"
    );
    let pending = Pending::new(id(3), Some(id(2))).expect("valid");
    assert_eq!(
        (pending.id(), pending.base(), pending.scope()),
        (id(3), Some(id(2)), 0)
    );
    assert_eq!(Manifest::new(id(1), Some(id(1)), 0), None);
    assert_eq!(
        Manifest::new(Manifest::default().id(), None, 0),
        None,
        "the placeholder id"
    );
    let manifest = Manifest::new(id(4), None, 8).expect("valid");
    assert_eq!(
        (manifest.id(), manifest.base(), manifest.checksum()),
        (id(4), None, 8)
    );
}
