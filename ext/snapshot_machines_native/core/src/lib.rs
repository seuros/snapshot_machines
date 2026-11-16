#![no_std]
#![cfg_attr(not(test), deny(clippy::indexing_slicing))]
#![doc = include_str!("../README.md")]

mod refusal;
mod types;

use periscope_machines::Issuer;

pub use refusal::Refusal;
pub use types::{Aborting, Committing, Manifest, Pending, SnapshotId};

/// The latest attempt: open, or the one most recently aborted. A committed
/// attempt is the chain's latest manifest, so it needs no record here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Attempt {
    Open(Pending),
    Aborted(Pending),
}

/// A bounded linear full-plus-incremental manifest chain.
///
/// `N` bounds the chain: one full manifest and up to `N - 1` incrementals.
/// Every id comes from one [`periscope_machines::Issuer`], so a catalog's
/// attempts, manifests and bases share one sequence that is never reissued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog<const N: usize> {
    manifests: [Option<Manifest>; N],
    len: usize,
    attempt: Option<Attempt>,
    ids: Issuer,
}

impl<const N: usize> Default for Catalog<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Catalog<N> {
    /// Start an empty catalog in scope zero.
    #[must_use]
    pub const fn new() -> Self {
        Self::with_scope(0)
    }

    /// Start an empty catalog issuing ids that carry `scope`.
    ///
    /// Two catalogs in the same scope issue the same ids, and an id handed
    /// to the wrong one can name an attempt or manifest there. Give each
    /// catalog whose ids could meet its own scope: an id from another scope
    /// is then always [`Refusal::Stale`], refused before anything is looked
    /// up. Keeping scopes distinct is the caller's job.
    #[must_use]
    pub const fn with_scope(scope: u16) -> Self {
        Self::issuing(Issuer::new(scope))
    }

    /// Rebuild a catalog after a restart from what an earlier one in `scope`
    /// saved: its [`Self::issued`] count and its chain, oldest first (the
    /// [`Self::restore_plan`] of its [`Self::latest`]).
    ///
    /// Built with [`Self::with_scope`] instead, the replacement would count
    /// from one again and an attempt that outlived the restart could commit
    /// into the new chain. Here the sequence continues after `issued`, so no
    /// id is reissued. No attempt is restored: one that was open or aborted
    /// is [`Refusal::Stale`], and an id issued but not in the chain is
    /// [`Refusal::NotLatest`] as a base. A repeated commit of a manifest
    /// still in the chain is honoured as before. An empty `chain` restores
    /// only the sequence.
    ///
    /// `None` for saved state that cannot exist: `issued` beyond
    /// [`periscope_machines::SEQUENCE_MAX`], more than `N` manifests, a
    /// manifest id of another scope or past `issued`, a first manifest that
    /// is not full, or a manifest whose base is not the one before it.
    #[must_use]
    pub fn restored(scope: u16, issued: u64, chain: &[Manifest]) -> Option<Self> {
        if chain.len() > N {
            return None;
        }
        let mut catalog = Self::issuing(Issuer::resumed(scope, issued)?);
        let mut parent = None;
        for (slot, manifest) in catalog.manifests.iter_mut().zip(chain) {
            if !catalog.ids.owns(manifest.id().raw()) || manifest.base() != parent {
                return None;
            }
            *slot = Some(*manifest);
            parent = Some(manifest.id());
        }
        catalog.len = chain.len();
        Some(catalog)
    }

    /// The scope carried by every id this catalog issues.
    #[must_use]
    pub const fn scope(&self) -> u16 {
        self.ids.scope()
    }

    /// How many ids this catalog has issued: the count to save for
    /// [`Self::restored`].
    #[must_use]
    pub const fn issued(&self) -> u64 {
        self.ids.issued()
    }

    /// The most manifests the chain holds (`N`).
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Begin a full checkpoint (`base` is `None`), or an incremental of the
    /// latest committed manifest.
    ///
    /// # Errors
    ///
    /// Checked in this order: [`Refusal::Stale`] when `base` was not issued
    /// by this catalog; [`Refusal::Busy`] while an attempt is open;
    /// [`Refusal::NotLatest`] when `base` is not the latest committed
    /// manifest; [`Refusal::ChainFull`] for an incremental of a full chain,
    /// or any checkpoint when `N` is zero; [`Refusal::Exhausted`] when no id
    /// is left. Nothing changes and no id is consumed.
    pub fn begin(&mut self, base: Option<SnapshotId>) -> Result<Pending, Refusal> {
        if let Some(base) = base {
            self.owned(base)?;
        }
        if matches!(self.attempt, Some(Attempt::Open(_))) {
            return Err(Refusal::Busy);
        }
        if base.is_some() && self.latest().map(Manifest::id) != base {
            return Err(Refusal::NotLatest);
        }
        let used = if base.is_some() { self.len } else { 0 };
        if used >= N {
            return Err(Refusal::ChainFull);
        }
        // The last thing that can refuse, so a refused begin consumes no id.
        let id = SnapshotId::issued(self.ids.issue().map_err(spent)?);
        // `base` is the latest manifest, issued before `id` in this scope.
        let pending = Pending::issued(id, base);
        self.attempt = Some(Attempt::Open(pending));
        Ok(pending)
    }

    /// Publish the open attempt after its data is durably written and
    /// verified. A full checkpoint replaces the whole chain; an incremental
    /// appends one link.
    ///
    /// Committing an attempt again with the same `checksum`, while its
    /// manifest is still in the chain, is honoured as
    /// [`Committing::AlreadyCommitted`] with the recorded manifest.
    ///
    /// # Errors
    ///
    /// [`Refusal::Stale`] when `pending` was not issued by this catalog, was
    /// aborted, or its manifest has left the chain; [`Refusal::Mismatch`]
    /// when its id is open or committed with a different base or checksum.
    /// Nothing changes.
    pub fn commit(&mut self, pending: Pending, checksum: u64) -> Result<Committing, Refusal> {
        self.owned(pending.id())?;
        match self.attempt {
            Some(Attempt::Open(open)) if open.id() == pending.id() => {
                if open != pending {
                    return Err(Refusal::Mismatch);
                }
                return self.publish(open, checksum);
            }
            Some(Attempt::Open(_) | Attempt::Aborted(_)) | None => {}
        }
        let recorded = self
            .chain()
            .iter()
            .flatten()
            .find(|entry| entry.id() == pending.id());
        match recorded {
            Some(entry) if entry.base() == pending.base() && entry.checksum() == checksum => {
                Ok(Committing::AlreadyCommitted(*entry))
            }
            Some(_) => Err(Refusal::Mismatch),
            None => Err(Refusal::Stale),
        }
    }

    /// Close the open attempt without touching committed manifests. Its id
    /// is never reused, so a late commit of it is refused.
    ///
    /// Aborting the most recently aborted attempt again is honoured as
    /// [`Aborting::AlreadyAborted`]: the catalog remembers that one attempt.
    ///
    /// # Errors
    ///
    /// [`Refusal::Stale`] when `pending` was not issued by this catalog, was
    /// committed, or was aborted before the latest attempt began;
    /// [`Refusal::Mismatch`] when its id is the latest attempt but its base
    /// differs. Nothing changes.
    pub fn abort(&mut self, pending: Pending) -> Result<Aborting, Refusal> {
        self.owned(pending.id())?;
        match self.attempt {
            Some(Attempt::Open(open)) if open.id() == pending.id() => {
                if open != pending {
                    return Err(Refusal::Mismatch);
                }
                self.attempt = Some(Attempt::Aborted(open));
                Ok(Aborting::Aborted)
            }
            Some(Attempt::Aborted(aborted)) if aborted.id() == pending.id() => {
                if aborted != pending {
                    return Err(Refusal::Mismatch);
                }
                Ok(Aborting::AlreadyAborted)
            }
            Some(Attempt::Open(_) | Attempt::Aborted(_)) | None => Err(Refusal::Stale),
        }
    }

    /// The attempt currently open, if any.
    #[must_use]
    pub const fn pending(&self) -> Option<Pending> {
        match self.attempt {
            Some(Attempt::Open(open)) => Some(open),
            Some(Attempt::Aborted(_)) | None => None,
        }
    }

    /// Latest committed manifest.
    #[must_use]
    pub const fn latest(&self) -> Option<Manifest> {
        // `len` never passes `N`; a miss would be no chain, as `chain` reads it.
        match self.manifests.split_at_checked(self.len) {
            Some((chain, _)) => match chain.last() {
                Some(latest) => *latest,
                None => None,
            },
            None => None,
        }
    }

    /// Copy the full-to-target restore chain into `out`, oldest first, and
    /// return how many manifests it holds. Writes the first `n` entries and
    /// leaves the rest untouched.
    ///
    /// # Errors
    ///
    /// [`Refusal::Stale`] when `target` was not issued by this catalog or is
    /// not in the chain; [`Refusal::Undersized`] when `out` is shorter than
    /// the plan. `out` is untouched.
    pub fn restore_plan(&self, target: SnapshotId, out: &mut [Manifest]) -> Result<usize, Refusal> {
        self.owned(target)?;
        let chain = self.chain();
        let at = chain
            .iter()
            .position(|entry| entry.is_some_and(|entry| entry.id() == target))
            .ok_or(Refusal::Stale)?;
        // `at` is a position in `chain`, so `at + 1` is at most its length.
        let planned = at + 1;
        let Some(room) = out.get_mut(..planned) else {
            return Err(Refusal::Undersized);
        };
        let mut written = 0;
        for (slot, entry) in room.iter_mut().zip(chain.iter().take(planned).flatten()) {
            *slot = *entry;
            // At most `planned`.
            written += 1;
        }
        Ok(written)
    }

    const fn issuing(ids: Issuer) -> Self {
        Self {
            manifests: [None; N],
            len: 0,
            attempt: None,
            ids,
        }
    }

    /// [`Refusal::Stale`] unless this catalog issued `id`: checked before
    /// anything is looked up.
    const fn owned(&self, id: SnapshotId) -> Result<(), Refusal> {
        if self.ids.owns(id.raw()) {
            Ok(())
        } else {
            Err(Refusal::Stale)
        }
    }

    /// The committed manifests, oldest first.
    fn chain(&self) -> &[Option<Manifest>] {
        self.manifests.get(..self.len).unwrap_or(&[])
    }

    /// Record the open attempt as a manifest.
    fn publish(&mut self, open: Pending, checksum: u64) -> Result<Committing, Refusal> {
        let full = open.base().is_none();
        let slot = if full { 0 } else { self.len };
        let manifest = Manifest::committed(open, checksum);
        let Some(entry) = self.manifests.get_mut(slot) else {
            debug_assert!(false, "begin left room in the chain for the open attempt");
            return Err(Refusal::ChainFull);
        };
        *entry = Some(manifest);
        if full {
            // A full snapshot starts a new chain at slot zero.
            self.manifests
                .iter_mut()
                .skip(1)
                .for_each(|old| *old = None);
        }
        // `slot < N`, as `get_mut` just proved.
        self.len = slot + 1;
        self.attempt = None;
        Ok(Committing::Committed(manifest))
    }
}

/// The issuer's only refusal is this core's [`Refusal::Exhausted`].
const fn spent(refusal: periscope_machines::Refusal) -> Refusal {
    match refusal {
        periscope_machines::Refusal::Exhausted => Refusal::Exhausted,
    }
}

#[cfg(test)]
mod tests;
