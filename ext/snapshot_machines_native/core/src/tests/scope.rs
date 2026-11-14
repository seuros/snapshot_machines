//! Scopes and restarts: a handle from another catalog names nothing here.

use super::*;

#[test]
fn a_catalog_refuses_another_scopes_pending() {
    let mut a = Catalog::<2>::with_scope(1);
    let mut b = Catalog::<2>::with_scope(2);
    assert_eq!((a.scope(), b.scope()), (1, 2));
    let foreign = begun(&mut a, None);
    let local = begun(&mut b, None);
    assert_eq!(foreign.scope(), 1);
    assert_eq!(foreign.id().raw() & periscope_machines::SEQUENCE_MAX, 1);
    assert_eq!(local.id().raw() & periscope_machines::SEQUENCE_MAX, 1);
    refuses(&mut b, Refusal::Stale, |c| c.commit(foreign, 1));
    refuses(&mut b, Refusal::Stale, |c| c.abort(foreign));
    committed(&mut b, local, 1);
}

#[test]
fn a_catalog_refuses_another_scopes_ids_as_base_and_restore_target() {
    let mut a = Catalog::<2>::with_scope(5);
    let mut b = Catalog::<2>::new();
    let pa = begun(&mut a, None);
    let ma = committed(&mut a, pa, 1);
    let pb = begun(&mut b, None);
    let mb = committed(&mut b, pb, 2);
    assert_eq!(
        ma.id().raw() & periscope_machines::SEQUENCE_MAX,
        mb.id().raw()
    );
    refuses(&mut b, Refusal::Stale, |c| plan(c, ma.id()));
    refuses(&mut b, Refusal::Stale, |c| c.begin(Some(ma.id())));
    assert_eq!(plan(&b, mb.id()), Ok(std::vec![mb]));
}

/// A catalog in scope 9 with a full and one incremental committed and a
/// third attempt open, and the state it would save: `issued` and its chain.
fn saved() -> (Catalog<3>, Pending, u64, std::vec::Vec<Manifest>) {
    let mut before = Catalog::<3>::with_scope(9);
    let full = begun(&mut before, None);
    let full = committed(&mut before, full, 1);
    let delta = begun(&mut before, Some(full.id()));
    let delta = committed(&mut before, delta, 2);
    let survivor = begun(&mut before, Some(delta.id()));
    let chain = plan(&before, delta.id()).expect("the latest is in the chain");
    let issued = before.issued();
    (before, survivor, issued, chain)
}

#[test]
fn a_restored_catalog_keeps_the_chain_and_refuses_handles_from_before_the_restart() {
    let (before, survivor, issued, chain) = saved();
    let mut after = Catalog::<3>::restored(9, issued, &chain).expect("saved state");
    assert_eq!(
        (
            after.scope(),
            after.issued(),
            after.latest(),
            after.pending()
        ),
        (9, issued, before.latest(), None)
    );
    assert_eq!(plan(&after, chain[1].id()), Ok(chain.clone()));
    refuses(&mut after, Refusal::Stale, |c| c.commit(survivor, 3));
    refuses(&mut after, Refusal::Stale, |c| c.abort(survivor));
    refuses(&mut after, Refusal::NotLatest, |c| {
        c.begin(Some(chain[0].id()))
    });
    let repeat = Pending::new(chain[1].id(), chain[1].base()).expect("valid");
    // A manifest still in the chain proves its repeat.
    let again = Committing::AlreadyCommitted(chain[1]);
    assert_eq!(unchanged(&mut after, |c| c.commit(repeat, 2)), Ok(again));
    let next = begun(&mut after, Some(chain[1].id()));
    assert!(
        next.id().raw() > survivor.id().raw(),
        "an id was reissued after the restart"
    );
    committed(&mut after, next, 3);
    // An empty chain restores only the sequence.
    let mut bare = Catalog::<3>::restored(9, issued, &[]).expect("saved state");
    assert_eq!(bare.latest(), None);
    refuses(&mut bare, Refusal::Stale, |c| plan(c, chain[0].id()));
    refuses(&mut bare, Refusal::NotLatest, |c| {
        c.begin(Some(chain[1].id()))
    });
}

#[test]
fn saved_state_that_cannot_exist_is_refused() {
    let (_, _, issued, chain) = saved();
    let [full, delta] = [chain[0], chain[1]];
    let orphan = Manifest::new(id((9 << 48) | 3), Some(full.id()), 0).expect("valid");
    let cases: [(&str, u16, u64, &[Manifest]); 6] = [
        (
            "issued past the sequence",
            9,
            periscope_machines::SEQUENCE_MAX + 1,
            &[],
        ),
        ("another scope", 8, issued, &chain),
        ("ids past issued", 9, 1, &chain),
        ("not starting with a full", 9, issued, &[delta]),
        (
            "a base that is not the one before",
            9,
            issued,
            &[full, delta, orphan],
        ),
        ("two fulls", 9, issued, &[full, full]),
    ];
    for (why, scope, issued, chain) in cases {
        assert_eq!(Catalog::<3>::restored(scope, issued, chain), None, "{why}");
    }
    assert_eq!(
        Catalog::<1>::restored(9, issued, &chain),
        None,
        "more manifests than capacity"
    );
}
