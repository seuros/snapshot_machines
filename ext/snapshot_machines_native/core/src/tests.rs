extern crate std;

use super::*;
use matryona_machines::testing::{refuses, unchanged};

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
