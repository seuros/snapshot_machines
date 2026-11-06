#![no_std]
#![doc = include_str!("../README.md")]

/// Catalog-scoped checkpoint identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SnapshotId(pub u64);
/// An in-progress checkpoint, issued by one catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pending {
    id: SnapshotId,
    base: Option<SnapshotId>,
}
impl Pending {
    /// Identity for external data storage.
    #[must_use]
    pub const fn id(self) -> SnapshotId {
        self.id
    }
}
/// A committed full checkpoint or one incremental link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Manifest {
    /// Checkpoint identity.
    pub id: SnapshotId,
    /// Parent, or `None` for a full checkpoint.
    pub base: Option<SnapshotId>,
    /// Caller-computed data integrity metadata.
    pub checksum: u64,
}
/// A bounded linear full-plus-incremental manifest chain.
#[derive(Debug)]
pub struct Catalog<const N: usize> {
    manifests: [Option<Manifest>; N],
    len: usize,
    sequence: u64,
    pending: Option<Pending>,
}
impl<const N: usize> Default for Catalog<N> {
    fn default() -> Self {
        Self::new()
    }
}
impl<const N: usize> Catalog<N> {
    /// Start an empty catalog in a fresh identity namespace.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            manifests: [None; N],
            len: 0,
            sequence: 0,
            pending: None,
        }
    }
    /// Begin a full checkpoint, or an incremental of the latest committed one.
    pub fn begin(&mut self, base: Option<SnapshotId>) -> Option<Pending> {
        if N == 0 || self.pending.is_some() {
            return None;
        }
        if let Some(base) = base
            && (self.len == N || self.latest().map(|entry| entry.id) != Some(base))
        {
            return None;
        }
        let sequence = self.sequence.checked_add(1)?;
        let pending = Pending {
            id: SnapshotId(sequence),
            base,
        };
        self.sequence = sequence;
        self.pending = Some(pending);
        Some(pending)
    }
    /// Publish a matching attempt after its data is durably written and verified.
    pub fn commit(&mut self, pending: Pending, checksum: u64) -> Option<Manifest> {
        if self.pending != Some(pending) {
            return None;
        }
        let manifest = Manifest {
            id: pending.id,
            base: pending.base,
            checksum,
        };
        if pending.base.is_none() {
            self.manifests = [None; N];
            self.len = 0;
        }
        self.manifests[self.len] = Some(manifest);
        self.len += 1;
        self.pending = None;
        Some(manifest)
    }
    /// Abort without deleting committed manifests or reusing the attempt ID.
    #[must_use]
    pub fn abort(&mut self, pending: Pending) -> bool {
        if self.pending != Some(pending) {
            return false;
        }
        self.pending = None;
        true
    }
    /// Latest committed checkpoint.
    #[must_use]
    pub const fn latest(&self) -> Option<Manifest> {
        if self.len == 0 {
            None
        } else {
            self.manifests[self.len - 1]
        }
    }
    /// Copy a complete full-to-target restore chain; unused slots are `None`.
    #[must_use]
    pub fn restore_plan(&self, target: SnapshotId) -> Option<[Option<Manifest>; N]> {
        let end = self.manifests[..self.len]
            .iter()
            .position(|entry| entry.is_some_and(|entry| entry.id == target))?;
        let mut plan = [None; N];
        plan[..=end].copy_from_slice(&self.manifests[..=end]);
        Some(plan)
    }
}
#[cfg(test)]
mod tests;
