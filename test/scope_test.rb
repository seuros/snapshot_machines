# frozen_string_literal: true

require_relative 'test_helper'

class ScopeTest < Minitest::Test
  include CatalogHelpers

  def test_another_scopes_pending_is_stale_not_an_exception
    a = Catalog.with_scope(1)
    b = Catalog.with_scope(2)
    foreign = a.begin(nil)
    local = b.begin(nil)
    assert_equal [1, 2], [foreign.id.scope, local.id.scope]
    assert_refused(:stale, b) { b.commit(foreign, 1) }
    assert_refused(:stale, b) { b.abort(foreign) }
    assert_instance_of Committed, b.commit(local, 1)
  end

  def test_another_scopes_ids_name_nothing
    a = Catalog.with_scope(5)
    b = Catalog.new
    theirs = full(a, 1)
    ours = full(b, 2)
    assert_refused(:stale, b, ours.id) { b.restore_plan(theirs.id) }
    assert_refused(:stale, b, ours.id) { b.begin(theirs.id) }
  end

  # A catalog in scope 9 with a full and an incremental committed and a third
  # attempt open, and what it saves: `issued` and its chain, oldest first.
  def saved
    before = Catalog.with_scope(9)
    monday = before.commit(before.begin(full(before, 1).id), 2).manifest
    [before, before.begin(monday.id), before.issued, before.restore_plan(monday.id)]
  end

  def test_restored_catalog_keeps_the_chain_and_refuses_handles_from_before_the_restart
    before, survivor, issued, chain = saved
    after = Catalog.restored(9, issued, chain)
    assert_equal [9, issued, before.latest, nil], [after.scope, after.issued, after.latest, after.pending]
    assert_equal chain, after.restore_plan(chain.last.id)
    assert_refused(:stale, after, chain.last.id) { after.commit(survivor, 3) }
    assert_refused(:stale, after, chain.last.id) { after.abort(survivor) }
    assert_refused(:not_latest, after, chain.last.id) { after.begin(chain.first.id) }
    repeat = Pending.new(chain.last.id, chain.last.base)
    assert_equal AlreadyCommitted.new(chain.last), after.commit(repeat, 2)
    assert_operator after.begin(chain.last.id).id.raw, :>, survivor.id.raw
    bare = Catalog.restored(9, issued, [])
    assert_nil bare.latest
    assert_refused(:not_latest, bare) { bare.begin(chain.last.id) }
  end

  def test_saved_state_that_cannot_exist_raises
    _, _, issued, chain = saved
    sunday, monday = chain
    orphan = Manifest.new(SnapshotId.new((9 << 48) | 3), sunday.id, 0)
    [
      [9, 2**48, []], [8, issued, chain], [9, 1, chain], [9, issued, [monday]],
      [9, issued, [sunday, monday, orphan]], [9, issued, [sunday, sunday]],
      [9, issued, chain * 17]
    ].each do |scope, count, manifests|
      assert_raises(RangeError) { Catalog.restored(scope, count, manifests) }
    end
    assert_raises(TypeError) { Catalog.restored(9, issued, chain.first) }
    assert_raises(TypeError) { Catalog.restored(9, issued, [chain.first.to_h]) }
    assert_raises(TypeError) { Catalog.restored(9, nil, []) }
  end

  def test_scope_validation
    assert_raises(TypeError) { Catalog.with_scope('1') }
    assert_raises(RangeError) { Catalog.with_scope(65_536) }
    assert_raises(RangeError) { Catalog.with_scope(-1) }
    assert_raises(TypeError) { Catalog.restored('9', 0, []) }
    assert_equal 0, Catalog.restored(65_535, 0, []).issued
  end
end
