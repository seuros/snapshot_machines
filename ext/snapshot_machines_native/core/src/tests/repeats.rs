//! Idempotent repeats: honoured when provable, refused when not.

use super::*;

#[test]
fn a_repeated_commit_with_the_same_payload_is_honoured_while_in_the_chain() {
    let mut catalog = Catalog::<3>::new();
    let pending = begun(&mut catalog, None);
    let manifest = committed(&mut catalog, pending, 7);
    let again = Committing::AlreadyCommitted(manifest);
    assert_eq!(unchanged(&mut catalog, |c| c.commit(pending, 7)), Ok(again));
    refuses(&mut catalog, Refusal::Mismatch, |c| c.commit(pending, 8));
    // Still provable once the chain has grown past it.
    let attempt = begun(&mut catalog, Some(manifest.id()));
    assert_eq!(unchanged(&mut catalog, |c| c.commit(pending, 7)), Ok(again));
    let delta = committed(&mut catalog, attempt, 9);
    let again = Committing::AlreadyCommitted(delta);
    assert_eq!(unchanged(&mut catalog, |c| c.commit(attempt, 9)), Ok(again));
    // A rebuilt attempt with the right id and the wrong base is not the same attempt.
    let forged = Pending::new(delta.id(), None).expect("valid");
    refuses(&mut catalog, Refusal::Mismatch, |c| c.commit(forged, 9));
    refuses(&mut catalog, Refusal::Stale, |c| c.abort(attempt));
    // Once a replacement commits, the old chain is gone and nothing proves it.
    let replacement = begun(&mut catalog, None);
    committed(&mut catalog, replacement, 1);
    refuses(&mut catalog, Refusal::Stale, |c| c.commit(pending, 7));
    refuses(&mut catalog, Refusal::Stale, |c| c.commit(attempt, 9));
}

#[test]
fn an_open_attempt_rebuilt_with_another_base_is_a_mismatch() {
    let mut catalog = Catalog::<2>::new();
    let full = begun(&mut catalog, None);
    let full = committed(&mut catalog, full, 1);
    let delta = begun(&mut catalog, Some(full.id()));
    let forged = Pending::new(delta.id(), None).expect("valid");
    refuses(&mut catalog, Refusal::Mismatch, |c| c.commit(forged, 2));
    refuses(&mut catalog, Refusal::Mismatch, |c| c.abort(forged));
    committed(&mut catalog, delta, 2);
}

#[test]
fn a_repeated_abort_is_honoured_for_the_latest_attempt_only() {
    let mut catalog = Catalog::<2>::new();
    let first = begun(&mut catalog, None);
    assert_eq!(catalog.abort(first), Ok(Aborting::Aborted));
    assert_eq!(
        unchanged(&mut catalog, |c| c.abort(first)),
        Ok(Aborting::AlreadyAborted)
    );
    refuses(&mut catalog, Refusal::Stale, |c| c.commit(first, 1));
    let forged = Pending::new(first.id(), None).expect("valid");
    // Rebuilt with the same id and base: the same attempt.
    assert_eq!(
        unchanged(&mut catalog, |c| c.abort(forged)),
        Ok(Aborting::AlreadyAborted)
    );
    let full = begun(&mut catalog, None);
    let full = committed(&mut catalog, full, 1);
    let rebased = Pending::new(first.id(), None).expect("valid");
    // A newer attempt began: the catalog no longer remembers the first abort.
    refuses(&mut catalog, Refusal::Stale, |c| c.abort(rebased));
    let second = begun(&mut catalog, Some(full.id()));
    assert_eq!(catalog.abort(second), Ok(Aborting::Aborted));
    let other_base = Pending::new(second.id(), None).expect("valid");
    refuses(&mut catalog, Refusal::Mismatch, |c| c.abort(other_base));
    assert_eq!(
        unchanged(&mut catalog, |c| c.abort(second)),
        Ok(Aborting::AlreadyAborted)
    );
}
