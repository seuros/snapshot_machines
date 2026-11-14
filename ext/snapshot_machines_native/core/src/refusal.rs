//! The one answer for every call that was not honoured.
use core::fmt;

/// Why a call was not honoured. Every refusal leaves the catalog exactly as
/// it was: no id consumed, no attempt opened or closed, no manifest moved.
///
/// A repeat the catalog can prove is not a refusal: committing an attempt
/// again with the payload it was committed with answers
/// [`Committing::AlreadyCommitted`](crate::Committing), aborting the most
/// recently aborted attempt again answers
/// [`Aborting::AlreadyAborted`](crate::Aborting). A repeat it cannot prove
/// (the manifest left the chain, an older attempt was aborted) is
/// [`Refusal::Stale`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// An attempt is already open: commit or abort it first.
    Busy,
    /// The chain has no room for the manifest: an incremental at capacity,
    /// or any checkpoint in a catalog of capacity zero.
    ChainFull,
    /// The base is an id this catalog issued, but not its latest committed
    /// manifest: superseded, replaced or aborted.
    NotLatest,
    /// The catalog's id sequence is spent: a scope holds 2^48 - 1 ids.
    Exhausted,
    /// The id names nothing current here: issued in another scope or not yet
    /// reached, an attempt that is no longer open and whose repeat cannot be
    /// proven, or a restore target that is not in the chain.
    Stale,
    /// The id names a recorded attempt or manifest, but the call carries a
    /// different payload (base or checksum); the record stands.
    Mismatch,
    /// The output buffer is shorter than the restore plan.
    Undersized,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Busy => "an attempt is already open",
            Self::ChainFull => "the chain has no room for the manifest",
            Self::NotLatest => "the base is not the latest committed manifest",
            Self::Exhausted => "id space exhausted",
            Self::Stale => "the id names nothing current in this catalog",
            Self::Mismatch => "the id is recorded with a different payload",
            Self::Undersized => "the output buffer is shorter than the restore plan",
        })
    }
}

impl core::error::Error for Refusal {}
