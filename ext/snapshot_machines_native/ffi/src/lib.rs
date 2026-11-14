//! Thin bindings for bounded manifest planning, never snapshot bytes.
mod classes;
mod init;
mod values;

use classes::Classes;
use magnus::{Error, RArray, Ruby, Value, prelude::*, typed_data::Obj};
use snapshot_machines::{Aborting, Catalog as Core, Manifest, Refusal};
use std::cell::RefCell;
use values::{Number, Scope};

const CAPACITY: usize = 32;

#[magnus::wrap(class = "SnapshotMachines::Catalog", free_immediately, size)]
struct Catalog {
    core: RefCell<Core<CAPACITY>>,
}

impl Catalog {
    const fn new() -> Self {
        Self::wrap(Core::new())
    }

    const fn with_scope(scope: Scope) -> Self {
        Self::wrap(Core::with_scope(scope.0))
    }

    /// `Catalog::restored(scope, issued, &chain)`. `TypeError` unless
    /// `chain` is an Array of `Manifest`s; `RangeError` for saved state the
    /// core refuses (it answers `None`).
    fn restored(ruby: &Ruby, scope: Scope, issued: Number, chain: Value) -> Result<Self, Error> {
        let chain = RArray::from_value(chain).ok_or_else(|| {
            Error::new(
                ruby.exception_type_error(),
                "chain must be an Array of SnapshotMachines::Manifest",
            )
        })?;
        let chain = chain
            .into_iter()
            .map(|manifest| values::manifest(ruby, manifest))
            .collect::<Result<Vec<Manifest>, Error>>()?;
        Core::restored(scope.0, issued.0, &chain)
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
            "begin",
            |core| core.begin(base),
            |pending| classes.pending(ruby, pending),
        )
    }

    fn commit(
        ruby: &Ruby,
        this: Obj<Self>,
        pending: Value,
        checksum: Number,
    ) -> Result<Value, Error> {
        this.check_frozen()?;
        let pending = values::pending(ruby, pending)?;
        let classes = Classes::get(ruby)?;
        Self::keep(
            ruby,
            this,
            "commit",
            |core| core.commit(pending, checksum.0),
            |committing| classes.committing(ruby, committing),
        )
    }

    /// Clone-then-keep: the core decides on a copy, under no borrow while
    /// Ruby builds the answer, and the copy is stored only once the answer
    /// exists and the catalog is still unfrozen and unchanged. A refusal
    /// changed nothing, so its symbol is answered without a store.
    fn keep<T>(
        ruby: &Ruby,
        this: Obj<Self>,
        call: &str,
        decide: impl FnOnce(&mut Core<CAPACITY>) -> Result<T, Refusal>,
        build: impl FnOnce(T) -> Result<Value, Error>,
    ) -> Result<Value, Error> {
        let before = this.core.borrow().clone();
        let mut after = before.clone();
        let honoured = match decide(&mut after) {
            Ok(honoured) => honoured,
            Err(refused) => return Ok(values::refusal(ruby, refused)),
        };
        let answer = build(honoured)?;
        this.check_frozen()?;
        let mut held = this.core.borrow_mut();
        if *held != before {
            return Err(Error::new(
                ruby.exception_runtime_error(),
                format!(
                    "the catalog changed while the answer was built; the {call} was not applied"
                ),
            ));
        }
        *held = after;
        drop(held);
        Ok(answer)
    }

    fn abort(ruby: &Ruby, this: Obj<Self>, pending: Value) -> Result<Value, Error> {
        this.check_frozen()?;
        let pending = values::pending(ruby, pending)?;
        let aborted = this.core.borrow_mut().abort(pending);
        Ok(match aborted {
            Ok(Aborting::Aborted) => ruby.to_symbol("aborted").as_value(),
            Ok(Aborting::AlreadyAborted) => ruby.to_symbol("already_aborted").as_value(),
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
