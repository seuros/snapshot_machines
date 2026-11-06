# frozen_string_literal: true
require_relative 'test_helper'
class CatalogTest < Minitest::Test
  def test_chain_replacement_abort_and_stale_handles
    catalog = SnapshotMachines::Catalog.new
    assert_equal 32, catalog.capacity
    first = catalog.begin(nil)
    assert_nil catalog.begin(nil)
    assert_equal [first.id, nil, 42], catalog.commit(first, 42)
    assert_nil catalog.commit(first, 99)
    second = catalog.begin(first.id)
    assert_equal [second.id, first.id, 43], catalog.commit(second, 43)
    assert_equal [[first.id, nil, 42], [second.id, first.id, 43]], catalog.restore_plan(second.id)
    attempt = catalog.begin(nil)
    assert catalog.abort(attempt)
    refute catalog.abort(attempt)
    assert_equal second.id, catalog.latest.first
    replacement = catalog.begin(nil)
    catalog.commit(replacement, 44)
    assert_nil catalog.restore_plan(first.id)
  end

  def test_scope_freeze_validation_and_gc
    assert_raises(TypeError) { SnapshotMachines::Pending.new }
    assert_raises(TypeError) { SnapshotMachines::Catalog.allocate }
    a = SnapshotMachines::Catalog.new
    b = SnapshotMachines::Catalog.new
    pending = a.begin(nil)
    assert_raises(TypeError) { pending.dup }
    b.begin(nil)
    assert_raises(ArgumentError) { b.commit(pending, 1) }
    assert_raises(ArgumentError) { b.abort(pending) }
    [1.1, '1', Object.new].each { |value| assert_raises(TypeError) { a.commit(pending, value) } }
    assert_raises(RangeError) { a.commit(pending, -1) }
    assert_raises(RangeError) { a.restore_plan(2**80) }
    GC.start
    GC.compact
    assert_equal [pending.id, nil, 1], a.commit(pending, 1)
    a.freeze
    assert_raises(FrozenError) { a.begin(nil) }
    assert_raises(FrozenError) { a.abort(pending) }
    assert_raises(FrozenError) { a.commit(pending, 2) }
    assert_equal 1, a.latest.last
  end

  def test_capacity_rejects_increment_but_allows_full_replacement
    catalog = SnapshotMachines::Catalog.new
    base = nil
    32.times do
      token = catalog.begin(base)
      base = catalog.commit(token, 0).first
    end
    assert_nil catalog.begin(base)
    refute_nil catalog.begin(nil)
  end
end
