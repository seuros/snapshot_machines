//! The gem's `Data` classes, resolved once at init. Every value read from
//! Ruby must be an instance of exactly one of them (by pointer) and is read
//! member by member without calling a reader, then re-validated by the core.
use std::sync::OnceLock;

use magnus::{
    Error, RClass, RModule, RStruct, Ruby, TryConvert, Value, prelude::*, rb_sys::AsRawValue,
    value::Opaque,
};
use snapshot_machines::{Committing, Manifest, Pending, SnapshotId};

/// `SnapshotId`, `Pending`, `Manifest`, `Committed` and `AlreadyCommitted`.
pub struct Classes {
    id: Opaque<RClass>,
    pending: Opaque<RClass>,
    manifest: Opaque<RClass>,
    committed: Opaque<RClass>,
    already_committed: Opaque<RClass>,
}

static CLASSES: OnceLock<Classes> = OnceLock::new();

fn runtime(ruby: &Ruby, message: &'static str) -> Error {
    Error::new(ruby.exception_runtime_error(), message)
}

/// Resolve the classes `lib/snapshot_machines.rb` defined before loading
/// the extension, and hold them for good: removing a constant later must
/// neither change what the catalog builds nor let the GC take a class.
pub fn define(ruby: &Ruby, module: RModule) -> Result<(), Error> {
    let class = |name: &str| -> Result<Opaque<RClass>, Error> {
        let class: RClass = module.const_get(name)?;
        ruby.gc_register_mark_object(class);
        Ok(class.into())
    };
    let classes = Classes {
        id: class("SnapshotId")?,
        pending: class("Pending")?,
        manifest: class("Manifest")?,
        committed: class("Committed")?,
        already_committed: class("AlreadyCommitted")?,
    };
    CLASSES.set(classes).map_err(|_| {
        runtime(
            ruby,
            "the snapshot_machines extension was initialised twice",
        )
    })
}

/// Whether `value` is an instance of exactly `class` (`instance_of?`), by
/// pointer, so no `==` the caller could redefine runs. A singleton class on
/// the instance itself does not change what it is.
fn instance_of(value: RStruct, class: RClass) -> bool {
    let actual = value.class();
    if actual.as_raw() == class.as_raw() {
        return true;
    }
    let singleton = value
        .singleton_class()
        .is_ok_and(|own| own.as_raw() == actual.as_raw());
    singleton
        && actual
            .superclass()
            .is_ok_and(|parent| parent.as_raw() == class.as_raw())
}

impl Classes {
    /// The classes, fetched before a core call so a failure changes nothing.
    pub fn get(ruby: &Ruby) -> Result<&'static Self, Error> {
        CLASSES
            .get()
            .ok_or_else(|| runtime(ruby, "the snapshot_machines extension is not initialised"))
    }

    /// `value` as a struct when it is exactly `class`: `TypeError` naming
    /// `name` otherwise, a subclass included.
    pub fn exact(
        ruby: &Ruby,
        value: Value,
        class: Opaque<RClass>,
        name: &str,
    ) -> Result<RStruct, Error> {
        RStruct::from_value(value)
            .filter(|members| instance_of(*members, ruby.get_inner(class)))
            .ok_or_else(|| {
                Error::new(
                    ruby.exception_type_error(),
                    format!("expected a SnapshotMachines::{name}"),
                )
            })
    }

    /// Member `index` of `value`, read without calling its reader.
    pub fn member<T: TryConvert>(value: RStruct, index: usize) -> Result<T, Error> {
        value.aref(index)
    }

    pub const fn id_class(&self) -> Opaque<RClass> {
        self.id
    }

    pub const fn pending_class(&self) -> Opaque<RClass> {
        self.pending
    }

    pub const fn manifest_class(&self) -> Opaque<RClass> {
        self.manifest
    }

    fn id(&self, ruby: &Ruby, id: SnapshotId) -> Result<Value, Error> {
        ruby.get_inner(self.id).funcall("new", (id.raw(),))
    }

    fn base(&self, ruby: &Ruby, base: Option<SnapshotId>) -> Result<Value, Error> {
        base.map_or_else(|| Ok(ruby.qnil().as_value()), |base| self.id(ruby, base))
    }

    pub fn pending(&self, ruby: &Ruby, pending: Pending) -> Result<Value, Error> {
        let id = self.id(ruby, pending.id())?;
        let base = self.base(ruby, pending.base())?;
        ruby.get_inner(self.pending).funcall("new", (id, base))
    }

    pub fn manifest(&self, ruby: &Ruby, manifest: Manifest) -> Result<Value, Error> {
        let id = self.id(ruby, manifest.id())?;
        let base = self.base(ruby, manifest.base())?;
        ruby.get_inner(self.manifest)
            .funcall("new", (id, base, manifest.checksum()))
    }

    pub fn committing(&self, ruby: &Ruby, committing: Committing) -> Result<Value, Error> {
        let (class, manifest) = match committing {
            Committing::Committed(manifest) => (self.committed, manifest),
            Committing::AlreadyCommitted(manifest) => (self.already_committed, manifest),
        };
        let manifest = self.manifest(ruby, manifest)?;
        ruby.get_inner(class).funcall("new", (manifest,))
    }
}
