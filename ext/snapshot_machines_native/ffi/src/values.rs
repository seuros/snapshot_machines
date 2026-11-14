//! Ruby values to core values, and the validating hooks behind every `Data`
//! class of the gem. A value is read only when it is exactly the gem's class,
//! and always re-validated by the core: a forged one raises before any core
//! call and never earns a refusal symbol.
use magnus::{Error, Integer, Ruby, TryConvert, Value, prelude::*};
use snapshot_machines::{Manifest, Pending, Refusal, SnapshotId};

use crate::classes::Classes;

/// `value` as an Integer converted to `T`: `TypeError` with `kind` for a
/// non-Integer, `RangeError` with `range` when it does not fit.
fn integer<T: TryConvert>(value: Value, kind: &str, range: &str) -> Result<T, Error> {
    let ruby = Ruby::get_with(value);
    if Integer::from_value(value).is_none() {
        return Err(Error::new(ruby.exception_type_error(), kind.to_owned()));
    }
    T::try_convert(value).map_err(|_| Error::new(ruby.exception_range_error(), range.to_owned()))
}

/// An Integer in `0..2**64`: `TypeError` for anything else, `RangeError`
/// for a negative or larger one.
#[derive(Clone, Copy)]
pub struct Number(pub u64);

impl TryConvert for Number {
    fn try_convert(value: Value) -> Result<Self, Error> {
        integer(
            value,
            "expected an Integer",
            "expected an Integer in 0..2**64",
        )
        .map(Self)
    }
}

/// A scope: an Integer in `0..=65535`.
#[derive(Clone, Copy)]
pub struct Scope(pub u16);

impl TryConvert for Scope {
    fn try_convert(value: Value) -> Result<Self, Error> {
        integer(
            value,
            "scope must be an Integer",
            "scope must be in 0..=65535",
        )
        .map(Self)
    }
}

/// The Ruby symbol of a refusal: the snake case of the core variant.
pub fn refusal(ruby: &Ruby, refused: Refusal) -> Value {
    ruby.to_symbol(match refused {
        Refusal::Busy => "busy",
        Refusal::ChainFull => "chain_full",
        Refusal::NotLatest => "not_latest",
        Refusal::Exhausted => "exhausted",
        Refusal::Stale => "stale",
        Refusal::Mismatch => "mismatch",
        Refusal::Undersized => "undersized",
    })
    .as_value()
}

fn zero_sequence(ruby: &Ruby) -> Error {
    Error::new(
        ruby.exception_range_error(),
        "a snapshot id's sequence (the low 48 bits) is never zero",
    )
}

/// `SnapshotId::from_raw`, `RangeError` on `None`.
fn id_of(ruby: &Ruby, raw: Number) -> Result<SnapshotId, Error> {
    SnapshotId::from_raw(raw.0).ok_or_else(|| zero_sequence(ruby))
}

/// The core id a Ruby `SnapshotId` names: `TypeError` for anything else.
pub fn snapshot_id(ruby: &Ruby, value: Value) -> Result<SnapshotId, Error> {
    let class = Classes::get(ruby)?.id_class();
    let value = Classes::exact(ruby, value, class, "SnapshotId")?;
    id_of(ruby, Classes::member(value, 0)?)
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
    let class = Classes::get(ruby)?.pending_class();
    let value = Classes::exact(ruby, value, class, "Pending")?;
    linked(ruby, Classes::member(value, 0)?, Classes::member(value, 1)?)
}

/// A manifest read from Ruby values; `RangeError` unless id and base link.
fn manifest_of(ruby: &Ruby, id: Value, base: Value, checksum: Number) -> Result<Manifest, Error> {
    let parts = linked(ruby, id, base)?;
    Manifest::new(parts.id(), parts.base(), checksum.0).ok_or_else(|| links(ruby))
}

/// The core manifest a Ruby `Manifest` names: `TypeError` for anything else.
pub fn manifest(ruby: &Ruby, value: Value) -> Result<Manifest, Error> {
    let class = Classes::get(ruby)?.manifest_class();
    let value = Classes::exact(ruby, value, class, "Manifest")?;
    manifest_of(
        ruby,
        Classes::member(value, 0)?,
        Classes::member(value, 1)?,
        Classes::member(value, 2)?,
    )
}

/// `SnapshotId.new(raw)`: `SnapshotId::from_raw`, `RangeError` on `None`.
pub fn checked_id(ruby: &Ruby, raw: Number) -> Result<u64, Error> {
    id_of(ruby, raw).map(SnapshotId::raw)
}

/// `SnapshotId#scope`.
pub fn id_scope(ruby: &Ruby, raw: Number) -> Result<u16, Error> {
    id_of(ruby, raw).map(SnapshotId::scope)
}

/// `Pending.new(id, base)`: `Pending::new`, `RangeError` on `None`.
pub fn checked_pending(ruby: &Ruby, id: Value, base: Value) -> Result<(), Error> {
    linked(ruby, id, base).map(|_| ())
}

/// `Manifest.new(id, base, checksum)`: `Manifest::new`, `RangeError` on `None`.
pub fn checked_manifest(
    ruby: &Ruby,
    id: Value,
    base: Value,
    checksum: Number,
) -> Result<(), Error> {
    manifest_of(ruby, id, base, checksum).map(|_| ())
}

/// `Committed.new(manifest)` and `AlreadyCommitted.new(manifest)`: exactly a
/// `Manifest`, re-validated by `Manifest::new` (a marshalled one included).
pub fn checked_holds_manifest(ruby: &Ruby, value: Value) -> Result<(), Error> {
    manifest(ruby, value).map(|_| ())
}
