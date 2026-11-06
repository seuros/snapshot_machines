//! Thin bindings for bounded manifest planning, never snapshot bytes.
mod init;

use magnus::{Error, Integer, RArray, Ruby, TryConvert, Value, prelude::*, typed_data::Obj};
use snapshot_machines::{Catalog, Manifest, Pending, SnapshotId};
use std::{cell::RefCell, sync::Arc};

const CAPACITY: usize = 32;

#[derive(Clone, Copy)]
struct Id(u64);
impl TryConvert for Id {
    fn try_convert(value: Value) -> Result<Self, Error> {
        if Integer::from_value(value).is_none() {
            return Err(Error::new(
                Ruby::get_with(value).exception_type_error(),
                "expected an Integer",
            ));
        }
        u64::try_convert(value).map(Self)
    }
}

#[magnus::wrap(class = "SnapshotMachines::Pending", free_immediately, size)]
struct Attempt {
    owner: Arc<()>,
    pending: Pending,
}
impl Attempt {
    const fn id(&self) -> u64 {
        self.pending.id().0
    }
}

#[magnus::wrap(class = "SnapshotMachines::Catalog", free_immediately, size)]
struct Wrapper {
    owner: Arc<()>,
    catalog: RefCell<Catalog<CAPACITY>>,
}

fn manifest(entry: Manifest) -> (u64, Option<u64>, u64) {
    (entry.id.0, entry.base.map(|id| id.0), entry.checksum)
}

impl Wrapper {
    fn new() -> Self {
        Self {
            owner: Arc::new(()),
            catalog: RefCell::new(Catalog::new()),
        }
    }
    #[allow(clippy::unused_self)] // Ruby instance query exposes the fixed bridge bound.
    const fn capacity(&self) -> usize {
        CAPACITY
    }
    fn begin(rb_self: Obj<Self>, base: Option<Id>) -> Result<Option<Attempt>, Error> {
        rb_self.check_frozen()?;
        let pending = rb_self
            .catalog
            .borrow_mut()
            .begin(base.map(|id| SnapshotId(id.0)));
        Ok(pending.map(|pending| Attempt {
            owner: Arc::clone(&rb_self.owner),
            pending,
        }))
    }
    fn commit(
        ruby: &Ruby,
        rb_self: Obj<Self>,
        attempt: &Attempt,
        checksum: Id,
    ) -> Result<Option<(u64, Option<u64>, u64)>, Error> {
        rb_self.check_frozen()?;
        if !Arc::ptr_eq(&rb_self.owner, &attempt.owner) {
            return Err(Error::new(
                ruby.exception_arg_error(),
                "pending handle belongs to another catalog",
            ));
        }
        let result = rb_self
            .catalog
            .borrow_mut()
            .commit(attempt.pending, checksum.0);
        Ok(result.map(manifest))
    }
    fn abort(ruby: &Ruby, rb_self: Obj<Self>, attempt: &Attempt) -> Result<bool, Error> {
        rb_self.check_frozen()?;
        if !Arc::ptr_eq(&rb_self.owner, &attempt.owner) {
            return Err(Error::new(
                ruby.exception_arg_error(),
                "pending handle belongs to another catalog",
            ));
        }
        Ok(rb_self.catalog.borrow_mut().abort(attempt.pending))
    }
    fn latest(&self) -> Option<(u64, Option<u64>, u64)> {
        self.catalog.borrow().latest().map(manifest)
    }
    fn restore_plan(ruby: &Ruby, rb_self: &Self, target: Id) -> Result<Option<RArray>, Error> {
        let plan = rb_self.catalog.borrow().restore_plan(SnapshotId(target.0));
        let Some(plan) = plan else { return Ok(None) };
        let result = ruby.ary_new();
        for entry in plan.into_iter().flatten() {
            result.push(manifest(entry))?;
        }
        Ok(Some(result))
    }
}
