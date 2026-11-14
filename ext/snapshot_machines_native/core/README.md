# snapshot-machines

> **AMS Chandy**, NCC-4218, *Chandy*-class archive tender.

Engine-room flight manual: chart checkpoint manifest ancestry, not snapshot bytes.

Allocation-free `no_std` linear checkpoint manifest planning. Begin a full or
incremental checkpoint, write/validate its data externally, then commit its
manifest. Incrementals must name the latest committed checkpoint as parent.
Aborted attempts consume IDs so stale completions cannot commit a replacement.

A committed full checkpoint replaces the old manifest chain; a pending/aborted
full checkpoint does not. Capacity exhaustion rejects incrementals and can be
resolved by successfully committing a full checkpoint. Restore plans return
the required full-to-target chain, not missing parent references. After a
restart, `Catalog::restored` rebuilds the chain from saved manifests.

The supplied checksum is opaque metadata: this core does not calculate or verify
it. There is no byte-diff engine, storage consistency boundary, serialization,
data garbage collection, or crash recovery adapter. Persist manifests and data
with a deployment-specific atomic publish protocol before deleting old data.

```rust
use snapshot_machines::{Catalog, Committing, Manifest};

let mut catalog = Catalog::<4>::with_scope(1);
let full = catalog.begin(None)?;
let sunday = match catalog.commit(full, 0xC0FFEE)? {
    Committing::Committed(manifest) | Committing::AlreadyCommitted(manifest) => manifest,
};
let monday = catalog.begin(Some(sunday.id()))?;
catalog.commit(monday, 0xBEEF)?;
let mut plan = [Manifest::default(); 4];
let n = catalog.restore_plan(monday.id(), &mut plan)?;
assert_eq!(n, 2);
assert_eq!(plan[0], sunday);
# Ok::<(), snapshot_machines::Refusal>(())
```

## Contract

### Refusals

Every operation that can refuse returns `Result<_, Refusal>`, and a refusal
changes nothing: no id consumed, no attempt opened or closed, no manifest
moved. One `Refusal` enum holds every reason:

| Call | `Ok` | `Err` |
| --- | --- | --- |
| `begin(base)` | `Pending` | `Stale`, `Busy`, `NotLatest`, `ChainFull`, `Exhausted` (checked in that order) |
| `commit(pending, checksum)` | `Committing::Committed`, `Committing::AlreadyCommitted` | `Stale`, `Mismatch` |
| `abort(pending)` | `Aborting::Aborted`, `Aborting::AlreadyAborted` | `Stale`, `Mismatch` |
| `restore_plan(target, out)` | manifests written to `out` | `Stale`, `Undersized` |

`latest`, `pending`, `scope`, `issued` and `capacity` are queries and never
refuse. A catalog of capacity zero answers `ChainFull` to every `begin`.

### Repeats

A repeat the catalog can prove from its own state is honoured and changes
nothing:

- `commit` of an attempt already committed, with the same base and checksum,
  while its manifest is still in the chain, answers
  `Committing::AlreadyCommitted` with the recorded manifest. Another checksum
  (or a rebuilt `Pending` with another base) is `Mismatch`. Once a full
  replacement has committed, the old chain is gone and the repeat is `Stale`.
- `abort` of the most recently aborted attempt answers
  `Aborting::AlreadyAborted`. The catalog remembers only that one: once a
  newer attempt begins, an older abort is `Stale`. Aborting a committed
  attempt is `Stale`; committing an aborted one is `Stale`.

### Scopes

Every id comes from one `periscope_machines::Issuer`: the scope in the top 16
bits of `SnapshotId::raw`, a sequence counted from one below, never reissued.
`Catalog::new()` is scope zero; `Catalog::with_scope(scope)` picks another.
Two catalogs in the same scope issue the same ids, so keeping scopes distinct
is the caller's job. A `Pending`, base or restore target this catalog never
issued (another scope, or a sequence not yet reached) is `Stale`, refused
before anything is looked up.

`SnapshotId::from_raw`, `Pending::new` and `Manifest::new` rebuild values
that crossed a boundary. They answer `None` for a zero sequence, or a base of
another scope or not issued before the id. Their fields are private, so no
value skips that check. `Manifest::default()` is an output buffer placeholder
whose id is raw zero; read only the entries reported. `SnapshotId` is not
ordered: ids of different scopes have no order between them.

### Restarts

A fresh catalog counts from one again, so an attempt that outlived a restart
could commit into the new chain. Save `Catalog::issued` before handing an id
out, and the chain whenever it changes (the `restore_plan` of `latest`, oldest
first). Build the replacement with
`Catalog::restored(scope, issued, &chain) -> Option<Self>`:

- the sequence continues after `issued`, so no id is reissued;
- the chain is rebuilt, so `latest`, `restore_plan`, an incremental of the
  latest manifest and a repeated commit of a manifest in the chain work as
  before the restart;
- no attempt is restored: one open or aborted before the restart is `Stale`;
- an empty chain restores only the sequence.

`restored` answers `None` for saved state that cannot exist: `issued` past
`periscope_machines::SEQUENCE_MAX`, more than `N` manifests, a manifest id of
another scope or past `issued`, a first manifest that is not full, or a
manifest whose base is not the one before it.

### Time

The catalog takes no time and records none.
