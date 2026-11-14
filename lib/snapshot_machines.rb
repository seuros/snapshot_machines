# frozen_string_literal: true

require_relative 'snapshot_machines/version'

# Every value mirrors the core type of the same name, field for field, and is
# validated by the core when it is built: a value built by hand is held to the
# same rule as one the catalog returned.
module SnapshotMachines
  # The core's `SnapshotId`: the identity of one attempt and of the manifest
  # it commits, issued once per catalog and never again. `raw` is its Integer
  # form, the catalog's scope in the top 16 bits above a sequence starting at
  # 1, so scope zero issues 1, 2, 3. `SnapshotId.new(raw)` is the core's
  # `SnapshotId::from_raw`: TypeError for a non-Integer, RangeError for a
  # negative one, one past 2**64 - 1, or a zero sequence (which no catalog
  # issues). A catalog answers :stale to an id it never issued.
  SnapshotId = Data.define(:raw) do
    def initialize(raw:)
      SnapshotMachines.__send__(:__snapshot_id, raw)
      super
    end

    # The scope of the catalog that issued this id.
    def scope = SnapshotMachines.__send__(:__scope_of, raw)
  end

  # The core's `Pending`: an open attempt, its SnapshotId and the SnapshotId
  # it builds on (nil for a full checkpoint). Compared by value, so it can be
  # stored and rebuilt. Like `Pending::new`: TypeError unless `id` is a
  # SnapshotId and `base` a SnapshotId or nil; RangeError when `base` is of
  # another scope or not issued before `id`.
  Pending = Data.define(:id, :base) do
    def initialize(id:, base:)
      SnapshotMachines.__send__(:__pending, id, base)
      super
    end
  end

  # The core's `Manifest`: a committed checkpoint, its SnapshotId, the
  # SnapshotId it builds on (nil for a full one) and the checksum you gave it
  # (an Integer in 0..2**64 - 1). Validated like `Manifest::new`, with the
  # same rules as Pending.
  Manifest = Data.define(:id, :base, :checksum) do
    def initialize(id:, base:, checksum:)
      SnapshotMachines.__send__(:__manifest, id, base, checksum)
      super
    end
  end

  # The check shared by both commit outcomes: `manifest` is exactly a
  # Manifest (TypeError otherwise, a subclass included), re-validated by the
  # core like one built by hand (RangeError).
  holds_a_manifest = proc do
    def initialize(manifest:)
      SnapshotMachines.__send__(:__holds_manifest, manifest)
      super
    end
  end

  # The core's `Committing::Committed(manifest)`: the attempt was published.
  Committed = Data.define(:manifest, &holds_a_manifest)

  # The core's `Committing::AlreadyCommitted(manifest)`: the attempt was
  # already committed with this checksum and is still in the chain; nothing
  # changed. Carries the recorded manifest.
  AlreadyCommitted = Data.define(:manifest, &holds_a_manifest)

  # Sealed: no subclass can skip the check by overriding `initialize`, and no
  # bare `allocate` builds a value that never ran it. Sealing stops accidents,
  # not reflection: the catalog reads only instances of exactly these classes
  # and re-validates each through the core.
  [SnapshotId, Pending, Manifest, Committed, AlreadyCommitted].each do |value|
    value.singleton_class.undef_method(:allocate)
    value.define_singleton_method(:inherited) { |_| raise TypeError, "#{value} cannot be subclassed" }
  end
end

# Native is required: there is no pure-Ruby fallback. It validates and builds
# the Data values above.
require 'snapshot_machines_native/snapshot_machines_native'

module SnapshotMachines
  private_class_method :__snapshot_id, :__scope_of, :__pending, :__manifest, :__holds_manifest
end
