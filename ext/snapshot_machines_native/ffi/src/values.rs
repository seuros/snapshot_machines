//! Ruby values to core values, and the validating hooks behind every `Data`
//! class of the gem. A value is read only when it is exactly the gem's class,
//! and always re-validated by the core: a forged one raises before any core
//! call and never earns a refusal symbol.
use magnus::{Error, Ruby, Value, prelude::*};
use matryona_machines::ruby::{Symbols, exact_fields, integer};
use snapshot_machines::{Aborting, Manifest, Pending, Refusal, SnapshotId};

use crate::classes::Classes;

matryona_machines::symbols! {
    /// The core's refusals as Symbols: the snake case of each variant.
    pub Refusals for Refusal as "a refusal" {
        Busy => "busy",
        ChainFull => "chain_full",
        NotLatest => "not_latest",
        Exhausted => "exhausted",
        Stale => "stale",
        Mismatch => "mismatch",
        Undersized => "undersized",
    }
}

matryona_machines::symbols! {
    /// The core's honoured `abort` answers as Symbols.
    pub Abortings for Aborting as "an abort" {
        Aborted => "aborted",
        AlreadyAborted => "already_aborted",
    }
}

/// The Ruby symbol of a refusal: the snake case of the core variant.
pub fn refusal(ruby: &Ruby, refused: Refusal) -> Value {
    Refusals::symbol(ruby, refused).as_value()
}

fn zero_sequence(ruby: &Ruby) -> Error {
    Error::new(
        ruby.exception_range_error(),
        "a snapshot id's sequence (the low 48 bits) is never zero",
    )
}

/// `SnapshotId::from_raw` on the Integer `raw`, `RangeError` on `None`.
fn id_of(ruby: &Ruby, raw: Value) -> Result<SnapshotId, Error> {
    SnapshotId::from_raw(integer(raw, "raw")?).ok_or_else(|| zero_sequence(ruby))
}

/// The core id a Ruby `SnapshotId` names: `TypeError` for anything else.
pub fn snapshot_id(ruby: &Ruby, value: Value) -> Result<SnapshotId, Error> {
    let class = Classes::get(ruby)?.id_class(ruby);
    let [raw] = exact_fields(
        ruby,
        value,
        class,
        "expected a SnapshotMachines::SnapshotId",
    )?;
    id_of(ruby, raw)
}

/// A base: `nil` or a Ruby `SnapshotId`.
pub fn base(ruby: &Ruby, value: Value) -> Result<Option<SnapshotId>, Error> {
    if value.is_nil() {
        return Ok(None);
    }
    snapshot_id(ruby, value).map(Some)
}

fn links(ruby: &Ruby) -> Error {
    Error::new(
        ruby.exception_range_error(),
        "base must be an id of the same scope issued before id",
    )
}

/// An id and a base read from Ruby values; `RangeError` unless they link.
fn linked(ruby: &Ruby, id: Value, base_value: Value) -> Result<Pending, Error> {
    let id = snapshot_id(ruby, id)?;
    let base = base(ruby, base_value)?;
    Pending::new(id, base).ok_or_else(|| links(ruby))
}

/// The core attempt a Ruby `Pending` names: `TypeError` for anything else.
pub fn pending(ruby: &Ruby, value: Value) -> Result<Pending, Error> {
    let class = Classes::get(ruby)?.pending_class(ruby);
    let [id, base] = exact_fields(ruby, value, class, "expected a SnapshotMachines::Pending")?;
    linked(ruby, id, base)
}

/// A manifest read from Ruby values; `RangeError` unless id and base link.
fn manifest_of(ruby: &Ruby, id: Value, base: Value, checksum: Value) -> Result<Manifest, Error> {
    let checksum = integer(checksum, "checksum")?;
    let parts = linked(ruby, id, base)?;
    Manifest::new(parts.id(), parts.base(), checksum).ok_or_else(|| links(ruby))
}

/// The core manifest a Ruby `Manifest` names: `TypeError` for anything else.
pub fn manifest(ruby: &Ruby, value: Value) -> Result<Manifest, Error> {
    let class = Classes::get(ruby)?.manifest_class(ruby);
    let [id, base, checksum] =
        exact_fields(ruby, value, class, "expected a SnapshotMachines::Manifest")?;
    manifest_of(ruby, id, base, checksum)
}

/// `SnapshotId.new(raw)`: `SnapshotId::from_raw`, `RangeError` on `None`.
pub fn checked_id(ruby: &Ruby, raw: Value) -> Result<u64, Error> {
    id_of(ruby, raw).map(SnapshotId::raw)
}

/// `SnapshotId#scope`.
pub fn id_scope(ruby: &Ruby, raw: Value) -> Result<u16, Error> {
    id_of(ruby, raw).map(SnapshotId::scope)
}

/// `Pending.new(id, base)`: `Pending::new`, `RangeError` on `None`.
pub fn checked_pending(ruby: &Ruby, id: Value, base: Value) -> Result<(), Error> {
    linked(ruby, id, base).map(|_| ())
}

/// `Manifest.new(id, base, checksum)`: `Manifest::new`, `RangeError` on `None`.
pub fn checked_manifest(ruby: &Ruby, id: Value, base: Value, checksum: Value) -> Result<(), Error> {
    manifest_of(ruby, id, base, checksum).map(|_| ())
}

/// `Committed.new(manifest)` and `AlreadyCommitted.new(manifest)`: exactly a
/// `Manifest`, re-validated by `Manifest::new` (a marshalled one included).
pub fn checked_holds_manifest(ruby: &Ruby, value: Value) -> Result<(), Error> {
    manifest(ruby, value).map(|_| ())
}
