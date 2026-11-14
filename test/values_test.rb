# frozen_string_literal: true

require_relative 'test_helper'

class ValuesTest < Minitest::Test
  include SnapshotMachines

  def test_snapshot_id_is_validated_by_the_core
    assert_equal 7, SnapshotId.new((7 << 48) | 1).scope
    assert_equal SnapshotId.new(3), SnapshotId.new(raw: 3)
    [0, 1 << 48, -1, 2**64].each { |raw| assert_raises(RangeError) { SnapshotId.new(raw) } }
    [1.0, '1', nil].each { |raw| assert_raises(TypeError) { SnapshotId.new(raw) } }
  end

  def test_pending_is_validated_by_the_core
    one = SnapshotId.new(1)
    two = SnapshotId.new(2)
    assert_equal two, Pending.new(two, one).id
    assert_nil Pending.new(one, nil).base
    assert_raises(RangeError) { Pending.new(one, two) }
    assert_raises(RangeError) { Pending.new(one, one) }
    assert_raises(RangeError) { Pending.new(SnapshotId.new((1 << 48) | 2), one) }
    assert_raises(TypeError) { Pending.new(1, nil) }
    assert_raises(TypeError) { Pending.new(two, 1) }
  end

  def test_manifest_is_validated_by_the_core
    one = SnapshotId.new(1)
    assert_equal 9, Manifest.new(SnapshotId.new(2), one, 9).checksum
    assert_raises(RangeError) { Manifest.new(one, SnapshotId.new(2), 9) }
    assert_raises(RangeError) { Manifest.new(one, nil, -1) }
    assert_raises(TypeError) { Manifest.new(one, nil, 1.5) }
    assert_raises(TypeError) { Manifest.new(1, nil, 1) }
  end

  def test_commit_outcomes_hold_a_manifest
    manifest = Manifest.new(SnapshotId.new(1), nil, 0)
    assert_equal manifest, Committed.new(manifest).manifest
    assert_equal manifest, AlreadyCommitted.new(manifest).manifest
    assert_raises(TypeError) { Committed.new(manifest.to_h) }
    assert_raises(TypeError) { AlreadyCommitted.new(nil) }
  end

  def test_values_are_sealed
    [SnapshotId, Pending, Manifest, Committed, AlreadyCommitted].each do |value|
      assert_raises(TypeError) { Class.new(value) { def initialize(**) = nil } }
      assert_raises(NoMethodError) { value.allocate }
    end
    assert_raises(RangeError) { SnapshotId.new(1).with(raw: 0) }
  end

  def test_hooks_are_private
    %i[__snapshot_id __scope_of __pending __manifest].each do |hook|
      refute_respond_to SnapshotMachines, hook
    end
  end
end
