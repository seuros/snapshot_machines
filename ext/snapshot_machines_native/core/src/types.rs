//! The values a catalog hands out and takes back.
use periscope_machines::{scope_of, sequence_of};

/// Identity of one checkpoint attempt, and of the manifest it commits.
///
/// Issued once per catalog and never again: the catalog's scope in the top
/// sixteen bits above a sequence counted from one, so scope zero issues 1, 2,
/// 3. It is a plain integer underneath ([`SnapshotId::raw`]), so it can cross
/// a queue or a database column and be rebuilt with
/// [`SnapshotId::from_raw`]; a rebuilt id the catalog never issued is
/// [`Refusal::Stale`](crate::Refusal::Stale) there. Every `SnapshotId` has a
/// sequence of at least one, except the one inside [`Manifest::default`].
///
/// Not ordered: ids of different scopes have no order between them, and
/// within one scope issue order is the catalog's business, not the caller's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SnapshotId(u64);

impl SnapshotId {
    /// Rebuild an id from its integer form.
    ///
    /// `None` when the sequence (the low 48 bits) is zero, which no catalog
    /// issues. Anything else is accepted here; the catalog decides whether
    /// it names anything.
    #[must_use]
    pub const fn from_raw(raw: u64) -> Option<Self> {
        if sequence_of(raw) == 0 {
            return None;
        }
        Some(Self(raw))
    }

    /// An id the catalog's issuer just handed out: counted from one.
    pub(crate) const fn issued(raw: u64) -> Self {
        Self(raw)
    }

    /// The integer form: scope above, sequence below.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// The scope of the catalog that issued this id.
    #[must_use]
    pub const fn scope(self) -> u16 {
        scope_of(self.0)
    }
}

/// Whether `base` can be the parent of `id`: both issued (sequence at least
/// one), same scope, and `base` issued first.
const fn links(id: SnapshotId, base: Option<SnapshotId>) -> bool {
    if sequence_of(id.0) == 0 {
        return false;
    }
    match base {
        None => true,
        Some(base) => sequence_of(base.0) != 0 && base.scope() == id.scope() && base.0 < id.0,
    }
}

/// An open checkpoint attempt: its id and the manifest it builds on.
///
/// Handed out by [`Catalog::begin`](crate::Catalog::begin) and compared by
/// value, so it can be stored and rebuilt with [`Pending::new`]. The catalog
/// refuses one it never issued: another scope's is
/// [`Refusal::Stale`](crate::Refusal::Stale) before anything is looked up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pending {
    id: SnapshotId,
    base: Option<SnapshotId>,
}

impl Pending {
    /// Rebuild an attempt from its parts, e.g. after it crossed a boundary.
    ///
    /// `None` when `id` was issued by no catalog, or `base` could not be its
    /// parent: a different scope, or an id not issued before `id`.
    #[must_use]
    pub const fn new(id: SnapshotId, base: Option<SnapshotId>) -> Option<Self> {
        if !links(id, base) {
            return None;
        }
        Some(Self { id, base })
    }

    /// An attempt the catalog just opened: `id` freshly issued, `base` its
    /// latest manifest, issued earlier in the same scope.
    pub(crate) const fn issued(id: SnapshotId, base: Option<SnapshotId>) -> Self {
        Self { id, base }
    }

    /// Identity for external data storage; the committed manifest's id.
    #[must_use]
    pub const fn id(self) -> SnapshotId {
        self.id
    }

    /// The manifest this attempt builds on; `None` for a full checkpoint.
    #[must_use]
    pub const fn base(self) -> Option<SnapshotId> {
        self.base
    }

    /// The scope of the catalog that issued this attempt.
    #[must_use]
    pub const fn scope(self) -> u16 {
        self.id.scope()
    }
}

/// A committed full checkpoint or one incremental link.
///
/// `Manifest::default()` is a placeholder for sizing the buffer handed to
/// [`Catalog::restore_plan`](crate::Catalog::restore_plan), e.g.
/// `[Manifest::default(); 8]`: its id is raw zero, which no catalog issues.
/// Read only the entries reported. Every other manifest is built by
/// [`Manifest::new`] or the catalog, so its base can be its parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Manifest {
    id: SnapshotId,
    base: Option<SnapshotId>,
    checksum: u64,
}

impl Manifest {
    /// Build a manifest from its parts, e.g. one read back from storage.
    ///
    /// `None` when `id` was issued by no catalog, or `base` could not be its
    /// parent: a different scope, or an id not issued before `id`.
    #[must_use]
    pub const fn new(id: SnapshotId, base: Option<SnapshotId>, checksum: u64) -> Option<Self> {
        if !links(id, base) {
            return None;
        }
        Some(Self { id, base, checksum })
    }

    /// The manifest the catalog publishes for `open`.
    pub(crate) const fn committed(open: Pending, checksum: u64) -> Self {
        Self {
            id: open.id,
            base: open.base,
            checksum,
        }
    }

    /// Checkpoint identity: the id of the attempt that committed it.
    #[must_use]
    pub const fn id(self) -> SnapshotId {
        self.id
    }

    /// Parent, or `None` for a full checkpoint.
    #[must_use]
    pub const fn base(self) -> Option<SnapshotId> {
        self.base
    }

    /// Caller-computed data integrity metadata.
    #[must_use]
    pub const fn checksum(self) -> u64 {
        self.checksum
    }
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            id: SnapshotId(0),
            base: None,
            checksum: 0,
        }
    }
}

/// Honoured answer to [`Catalog::commit`](crate::Catalog::commit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Committing {
    /// The open attempt was published as this manifest.
    Committed(Manifest),
    /// The attempt was already committed with this payload and its manifest
    /// is still in the chain; nothing changed. Carries the original answer.
    AlreadyCommitted(Manifest),
}

/// Honoured answer to [`Catalog::abort`](crate::Catalog::abort).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aborting {
    /// The open attempt was closed; its id is never reused.
    Aborted,
    /// The attempt is the one most recently aborted; nothing changed.
    AlreadyAborted,
}
