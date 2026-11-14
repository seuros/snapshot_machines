//! The gem's `Data` classes, resolved once at init and pinned. Every value
//! read from Ruby must be an instance of exactly one of them and is read
//! member by member without calling a reader, then re-validated by the core.
use magnus::{Error, RClass, RModule, Ruby, Value, prelude::*};
use matryona_machines::ruby::{Class, Registry};
use snapshot_machines::{Committing, Manifest, Pending, SnapshotId};

/// `SnapshotId`, `Pending`, `Manifest`, `Committed` and `AlreadyCommitted`.
pub struct Classes {
    id: Class,
    pending: Class,
    manifest: Class,
    committed: Class,
    already_committed: Class,
}

static CLASSES: Registry<Classes> = Registry::new("snapshot_machines");

/// Resolve the classes `lib/snapshot_machines.rb` defined before loading
/// the extension, and hold them for good: removing a constant later must
/// neither change what the catalog builds nor let the GC take a class.
pub fn define(ruby: &Ruby, module: RModule) -> Result<(), Error> {
    let class = |name| Class::resolve(ruby, module, name);
    let classes = Classes {
        id: class("SnapshotId")?,
        pending: class("Pending")?,
        manifest: class("Manifest")?,
        committed: class("Committed")?,
        already_committed: class("AlreadyCommitted")?,
    };
    CLASSES.set(ruby, classes).map(drop)
}

impl Classes {
    /// The classes, fetched before a core call so a failure changes nothing.
    pub fn get(ruby: &Ruby) -> Result<&'static Self, Error> {
        CLASSES.get(ruby)
    }

    pub fn id_class(&self, ruby: &Ruby) -> RClass {
        self.id.get(ruby)
    }

    pub fn pending_class(&self, ruby: &Ruby) -> RClass {
        self.pending.get(ruby)
    }

    pub fn manifest_class(&self, ruby: &Ruby) -> RClass {
        self.manifest.get(ruby)
    }

    fn id(&self, ruby: &Ruby, id: SnapshotId) -> Result<Value, Error> {
        self.id.get(ruby).funcall("new", (id.raw(),))
    }

    fn base(&self, ruby: &Ruby, base: Option<SnapshotId>) -> Result<Value, Error> {
        base.map_or_else(|| Ok(ruby.qnil().as_value()), |base| self.id(ruby, base))
    }

    pub fn pending(&self, ruby: &Ruby, pending: Pending) -> Result<Value, Error> {
        let id = self.id(ruby, pending.id())?;
        let base = self.base(ruby, pending.base())?;
        self.pending.get(ruby).funcall("new", (id, base))
    }

    pub fn manifest(&self, ruby: &Ruby, manifest: Manifest) -> Result<Value, Error> {
        let id = self.id(ruby, manifest.id())?;
        let base = self.base(ruby, manifest.base())?;
        self.manifest
            .get(ruby)
            .funcall("new", (id, base, manifest.checksum()))
    }

    pub fn committing(&self, ruby: &Ruby, committing: Committing) -> Result<Value, Error> {
        let (class, manifest) = match committing {
            Committing::Committed(manifest) => (self.committed, manifest),
            Committing::AlreadyCommitted(manifest) => (self.already_committed, manifest),
        };
        let manifest = self.manifest(ruby, manifest)?;
        class.get(ruby).funcall("new", (manifest,))
    }
}
