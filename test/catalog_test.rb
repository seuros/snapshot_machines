# frozen_string_literal: true

require_relative 'test_helper'

class CatalogTest < Minitest::Test
  include CatalogHelpers

  def test_chain_replacement_and_restore_plans
    catalog = Catalog.new
    assert_equal [32, 0, 0], [catalog.capacity, catalog.scope, catalog.issued]
    assert_equal [nil, nil], [catalog.pending, catalog.latest]
    first = catalog.begin(nil)
    assert_equal Pending.new(SnapshotId.new(1), nil), first
    assert_equal first, catalog.pending
    sunday = catalog.commit(first, 42)
    assert_equal Committed.new(Manifest.new(SnapshotId.new(1), nil, 42)), sunday
    monday = catalog.commit(catalog.begin(sunday.manifest.id), 43).manifest
    assert_equal Manifest.new(SnapshotId.new(2), SnapshotId.new(1), 43), monday
    assert_equal [sunday.manifest, monday], catalog.restore_plan(monday.id)
    replacement = catalog.begin(nil)
    assert_equal [sunday.manifest, monday], catalog.restore_plan(monday.id)
    fresh = catalog.commit(replacement, 44).manifest
    assert_equal :stale, catalog.restore_plan(sunday.manifest.id)
    assert_equal [fresh], catalog.restore_plan(fresh.id)
    assert_equal fresh, catalog.latest
  end

  def test_every_begin_refusal_is_a_symbol_and_changes_nothing
    catalog = Catalog.new
    assert_refused(:stale, catalog) { catalog.begin(SnapshotId.new(1)) }
    open = catalog.begin(nil)
    assert_refused(:busy, catalog) { catalog.begin(nil) }
    sunday = catalog.commit(open, 1).manifest
    monday = catalog.commit(catalog.begin(sunday.id), 2).manifest
    assert_refused(:not_latest, catalog, monday.id) { catalog.begin(sunday.id) }
    base = monday.id
    30.times { base = catalog.commit(catalog.begin(base), 0).manifest.id }
    assert_refused(:chain_full, catalog, base) { catalog.begin(base) }
    assert_instance_of Pending, catalog.begin(nil)
  end

  def test_exhausted_sequence
    catalog = Catalog.restored(0, (2**48) - 1, [])
    assert_refused(:exhausted, catalog) { catalog.begin(nil) }
  end

  def test_repeats_are_honoured_only_when_provable
    catalog = Catalog.new
    attempt = catalog.begin(nil)
    manifest = catalog.commit(attempt, 7).manifest
    before = state(catalog, manifest.id)
    assert_equal AlreadyCommitted.new(manifest), catalog.commit(attempt, 7)
    assert_equal before, state(catalog, manifest.id)
    assert_refused(:mismatch, catalog, manifest.id) { catalog.commit(attempt, 8) }
    assert_refused(:stale, catalog, manifest.id) { catalog.abort(attempt) }

    aborted = catalog.begin(manifest.id)
    assert_equal :aborted, catalog.abort(aborted)
    before = state(catalog, manifest.id)
    assert_equal :already_aborted, catalog.abort(aborted)
    assert_equal before, state(catalog, manifest.id)
    assert_refused(:mismatch, catalog) { catalog.abort(Pending.new(aborted.id, nil)) }
    assert_refused(:stale, catalog) { catalog.commit(aborted, 1) }
    catalog.abort(catalog.begin(nil))
    assert_refused(:stale, catalog) { catalog.abort(aborted) }
  end

  def test_malformed_input_raises_before_the_core_and_loses_nothing
    catalog = Catalog.new
    attempt = catalog.begin(nil)
    before = state(catalog)
    [1.1, '1', Object.new, nil].each { |value| assert_raises(TypeError) { catalog.commit(attempt, value) } }
    assert_raises(RangeError) { catalog.commit(attempt, -1) }
    assert_raises(RangeError) { catalog.commit(attempt, 2**64) }
    assert_raises(TypeError) { catalog.commit(attempt.to_h, 1) }
    assert_raises(TypeError) { catalog.abort(attempt.id) }
    assert_raises(TypeError) { catalog.begin(1) }
    assert_raises(TypeError) { catalog.restore_plan(1) }
    assert_equal before, state(catalog)
    assert_instance_of Committed, catalog.commit(attempt, 1)
  end

  def test_freeze_and_allocation
    assert_raises(TypeError) { Catalog.allocate }
    catalog = Catalog.new
    attempt = catalog.begin(nil)
    GC.start
    GC.compact
    catalog.freeze
    assert_raises(FrozenError) { catalog.begin(nil) }
    assert_raises(FrozenError) { catalog.abort(attempt) }
    assert_raises(FrozenError) { catalog.commit(attempt, 2) }
    assert_equal attempt, catalog.pending
  end
end
