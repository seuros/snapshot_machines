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
the required full-to-target chain, not missing parent references.

The supplied checksum is opaque metadata: this core does not calculate or verify
it. There is no byte-diff engine, storage consistency boundary, serialization,
data garbage collection, or crash recovery adapter. Persist manifests and data
with a deployment-specific atomic publish protocol before deleting old data.
IDs and pending handles are scoped to one catalog lifetime.
