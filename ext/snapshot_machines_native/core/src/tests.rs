extern crate std;

use super::*;

mod chain;
mod model;
mod repeats;
mod scope;

fn begun<const N: usize>(catalog: &mut Catalog<N>, base: Option<SnapshotId>) -> Pending {
    match catalog.begin(base) {
        Ok(pending) => pending,
        Err(refusal) => panic!("begin({base:?}) refused: {refusal:?}"),
    }
}

fn committed<const N: usize>(
    catalog: &mut Catalog<N>,
    pending: Pending,
    checksum: u64,
) -> Manifest {
    match catalog.commit(pending, checksum) {
        Ok(Committing::Committed(manifest)) => manifest,
        other => panic!("commit({pending:?}) answered {other:?}"),
    }
}

fn id(raw: u64) -> SnapshotId {
    SnapshotId::from_raw(raw).expect("a nonzero sequence")
}

/// The plan for `target`, as a vector of exactly the reported entries.
fn plan<const N: usize>(
    catalog: &Catalog<N>,
    target: SnapshotId,
) -> Result<std::vec::Vec<Manifest>, Refusal> {
    let mut out = [Manifest::default(); 64];
    let written = catalog.restore_plan(target, &mut out)?;
    Ok(out[..written].to_vec())
}

/// Run `call` on `catalog`, insisting it is refused with `refusal` and
/// changes nothing.
fn refused<const N: usize, T: core::fmt::Debug>(
    catalog: &mut Catalog<N>,
    refusal: Refusal,
    call: impl FnOnce(&mut Catalog<N>) -> Result<T, Refusal>,
) {
    let before = catalog.clone();
    let answer = call(catalog);
    match answer {
        Err(got) => assert_eq!(got, refusal),
        Ok(honoured) => panic!("expected {refusal:?}, got Ok({honoured:?})"),
    }
    assert_eq!(*catalog, before, "a refusal changed the catalog");
}

/// Run `call` on `catalog`, insisting it is honoured with `answer` (a repeat)
/// and changes nothing.
fn repeated<const N: usize, T: Copy + core::fmt::Debug + PartialEq>(
    catalog: &mut Catalog<N>,
    answer: T,
    call: impl FnOnce(&mut Catalog<N>) -> Result<T, Refusal>,
) {
    let before = catalog.clone();
    assert_eq!(call(catalog), Ok(answer));
    assert_eq!(*catalog, before, "an honoured repeat changed the catalog");
}
