# frozen_string_literal: true

require_relative 'test_helper'

# Sealing stops accidents, not reflection; the native boundary is the
# authority. A forged value raises before any core call and never earns a
# refusal symbol.
class ForgedTest < Minitest::Test
  include CatalogHelpers

  VALUES = { SnapshotId: { raw: 0 }, Pending: { id: 0, base: 0 }, Manifest: { id: 0, base: 0, checksum: -1 },
             Committed: { manifest: 0 }, AlreadyCommitted: { manifest: 0 } }.freeze

  # `class ForgedX < X; end` raises TypeError, yet Ruby binds the constant
  # before `inherited` runs: the subclass leaks. Its `initialize` skips the
  # check by binding `Data#initialize` directly to whatever members it gets.
  def forged(name, **members)
    forged_name = :"Forged#{name}"
    unless Object.const_defined?(forged_name)
      error = assert_raises(TypeError) do
        eval("class #{forged_name} < SnapshotMachines::#{name}; end", TOPLEVEL_BINDING, __FILE__, __LINE__)
      end
      assert_match(/cannot be subclassed/, error.message)
      Object.const_get(forged_name).class_eval do
        def initialize(**members) = Data.instance_method(:initialize).bind_call(self, **members)
      end
    end
    Object.const_get(forged_name).new(**members)
  end

  # `value` dumped, its bytes edited, and loaded: `Marshal.load` skips `initialize`.
  def remarshal(value, from, to)
    Marshal.load(Marshal.dump(value).sub(from.b, to.b))
  end

  def test_every_value_class_is_sealed_and_the_named_form_leaks
    VALUES.each do |name, members|
      value = SnapshotMachines.const_get(name)
      assert_raises(TypeError) { Class.new(value) }
      assert_raises(NoMethodError) { value.allocate }
      forgery = forged(name, **members)
      assert_kind_of value, forgery
      refute_instance_of value, forgery
    end
  end

  def test_a_forged_id_raises_type_error_and_changes_nothing
    catalog = Catalog.new
    sunday = full(catalog, 1)
    before = state(catalog, sunday.id)
    [forged(:SnapshotId, raw: 0), forged(:SnapshotId, raw: sunday.id.raw)].each do |id|
      assert_raises(TypeError) { catalog.begin(id) }
      assert_raises(TypeError) { catalog.restore_plan(id) }
      assert_raises(TypeError) { Pending.new(id, nil) }
      assert_raises(TypeError) { Manifest.new(SnapshotId.new(2), id, 0) }
    end
    assert_equal before, state(catalog, sunday.id)
  end

  def test_a_forged_pending_or_manifest_raises_type_error_and_changes_nothing
    catalog = Catalog.new
    attempt = catalog.begin(nil)
    before = state(catalog)
    fake = forged(:Pending, id: attempt.id, base: nil)
    assert_raises(TypeError) { catalog.commit(fake, 1) }
    assert_raises(TypeError) { catalog.abort(fake) }
    manifest = forged(:Manifest, id: attempt.id, base: nil, checksum: 1)
    assert_raises(TypeError) { Committed.new(manifest) }
    assert_raises(TypeError) { AlreadyCommitted.new(manifest) }
    assert_raises(TypeError) { Catalog.restored(0, 1, [manifest]) }
    assert_equal before, state(catalog)
  end

  def test_a_marshalled_value_is_revalidated_by_the_core
    catalog = Catalog.new
    sunday = full(catalog, 1)
    zero = remarshal(SnapshotId.new(1), "i\x06", "i\x00")
    assert_equal 0, zero.raw
    before = state(catalog, sunday.id)
    assert_raises(RangeError) { catalog.begin(zero) }
    assert_raises(RangeError) { catalog.restore_plan(zero) }
    unlinked = remarshal(Pending.new(SnapshotId.new(2), SnapshotId.new(1)), "i\x07", "i\x06")
    assert_raises(RangeError) { catalog.commit(unlinked, 1) }
    assert_raises(RangeError) { catalog.abort(unlinked) }
    assert_equal before, state(catalog, sunday.id)
    looped = remarshal(Manifest.new(SnapshotId.new(2), SnapshotId.new(1), 0), "i\x07", "i\x06")
    assert_raises(RangeError) { Committed.new(looped) }
    assert_raises(RangeError) { Catalog.restored(0, 2, [sunday, looped]) }
  end

  # Classes compare by pointer: a singleton class on a real value does not
  # change what it is, and a redefined `==` on a forged class fools nothing.
  def test_exact_class_ignores_singletons_and_redefined_equality
    catalog = Catalog.new
    sunday = full(catalog, 1)
    id = SnapshotId.new(sunday.id.raw)
    id.singleton_class
    assert_equal [sunday], catalog.restore_plan(id)
    fake = forged(:SnapshotId, raw: sunday.id.raw)
    fake.class.define_singleton_method(:==) { |_| true }
    assert_raises(TypeError) { catalog.restore_plan(fake) }
  end

  def test_hooks_are_private
    refute_respond_to SnapshotMachines, :__holds_manifest
  end
end
