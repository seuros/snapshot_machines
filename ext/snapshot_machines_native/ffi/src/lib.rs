//! Thin bindings for bounded manifest planning, never snapshot bytes.
mod classes;
mod init;
mod values;

use classes::Classes;
use magnus::{Error, Ruby, Value, prelude::*, typed_data::Obj};
use matryona_machines::ruby::{Symbols, exact_array, integer, keep};
use snapshot_machines::{Catalog as Core, Manifest, Refusal};
use std::cell::RefCell;

const CAPACITY: usize = 32;

#[magnus::wrap(class = "SnapshotMachines::Catalog", free_immediately, size)]
struct Catalog {
    core: RefCell<Core<CAPACITY>>,
}

impl Catalog {
    const fn new() -> Self {
        Self::wrap(Core::new())
    }

    fn with_scope(scope: Value) -> Result<Self, Error> {
        Ok(Self::wrap(Core::with_scope(integer(scope, "scope")?)))
    }

    /// `Catalog::restored(scope, issued, &chain)`. `TypeError` unless
    /// `chain` is exactly an Array (no subclass) of `Manifest`s; `RangeError`
    /// for saved state the core refuses (it answers `None`).
    fn restored(ruby: &Ruby, scope: Value, issued: Value, chain: Value) -> Result<Self, Error> {
        let (scope, issued) = (integer(scope, "scope")?, integer(issued, "issued")?);
        let chain = exact_array(
            ruby,
            chain,
            "chain must be an Array of SnapshotMachines::Manifest",
        )?
        .into_iter()
        .map(|manifest| values::manifest(ruby, manifest))
        .collect::<Result<Vec<Manifest>, Error>>()?;
        Core::restored(scope, issued, &chain)
            .map(Self::wrap)
            .ok_or_else(|| {
                Error::new(
                    ruby.exception_range_error(),
                    "saved state that cannot exist: issued past 2**48 - 1, more than 32 \
                     manifests, an id of another scope or past issued, or a chain that does \
                     not start full and link each base to the manifest before it",
                )
            })
    }

    const fn wrap(core: Core<CAPACITY>) -> Self {
        Self {
            core: RefCell::new(core),
        }
    }

    fn scope(&self) -> u16 {
        self.core.borrow().scope()
    }

    fn issued(&self) -> u64 {
        self.core.borrow().issued()
    }

    fn capacity(&self) -> usize {
        self.core.borrow().capacity()
    }

    fn begin(ruby: &Ruby, this: Obj<Self>, base: Value) -> Result<Value, Error> {
        this.check_frozen()?;
        let base = values::base(ruby, base)?;
        let classes = Classes::get(ruby)?;
        Self::keep(
            ruby,
            this,
            |core| core.begin(base),
            |pending| classes.pending(ruby, pending),
        )
    }

    fn commit(
        ruby: &Ruby,
        this: Obj<Self>,
        pending: Value,
        checksum: Value,
    ) -> Result<Value, Error> {
        this.check_frozen()?;
        let pending = values::pending(ruby, pending)?;
        let checksum = integer(checksum, "checksum")?;
        let classes = Classes::get(ruby)?;
        Self::keep(
            ruby,
            this,
            |core| core.commit(pending, checksum),
            |committing| classes.committing(ruby, committing),
        )
    }

    /// Clone-then-keep: the core decides on a copy, under no borrow while
    /// Ruby builds the answer, and the copy is stored only once the answer
    /// exists and the catalog is still unfrozen and unchanged. A refusal
    /// changed nothing, so its symbol is the answer.
    fn keep<T>(
        ruby: &Ruby,
        this: Obj<Self>,
        decide: impl FnOnce(&mut Core<CAPACITY>) -> Result<T, Refusal>,
        build: impl FnOnce(T) -> Result<Value, Error>,
    ) -> Result<Value, Error> {
        keep(ruby, this, &this.core, decide, |decided| match decided {
            Ok(honoured) => build(honoured),
            Err(refused) => Ok(values::refusal(ruby, refused)),
        })
    }

    fn abort(ruby: &Ruby, this: Obj<Self>, pending: Value) -> Result<Value, Error> {
        this.check_frozen()?;
        let pending = values::pending(ruby, pending)?;
        let aborted = this.core.borrow_mut().abort(pending);
        Ok(match aborted {
            Ok(aborting) => values::Abortings::symbol(ruby, aborting).as_value(),
            Err(refused) => values::refusal(ruby, refused),
        })
    }

    fn pending(ruby: &Ruby, this: &Self) -> Result<Option<Value>, Error> {
        let open = this.core.borrow().pending();
        open.map(|pending| Classes::get(ruby)?.pending(ruby, pending))
            .transpose()
    }

    fn latest(ruby: &Ruby, this: &Self) -> Result<Option<Value>, Error> {
        let latest = this.core.borrow().latest();
        latest
            .map(|manifest| Classes::get(ruby)?.manifest(ruby, manifest))
            .transpose()
    }

    fn restore_plan(ruby: &Ruby, this: &Self, target: Value) -> Result<Value, Error> {
        let target = values::snapshot_id(ruby, target)?;
        let classes = Classes::get(ruby)?;
        let mut out = [Manifest::default(); CAPACITY];
        let planned = this.core.borrow().restore_plan(target, &mut out);
        let written = match planned {
            Ok(written) => written,
            Err(refused) => return Ok(values::refusal(ruby, refused)),
        };
        let plan = ruby.ary_new_capa(written);
        for manifest in out.iter().take(written) {
            plan.push(classes.manifest(ruby, *manifest)?)?;
        }
        Ok(plan.as_value())
    }
}
