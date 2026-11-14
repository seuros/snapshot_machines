# frozen_string_literal: true

require_relative 'test_helper'

# `begin` and `commit` answer Ruby objects, built by calling `new`, which is
# Ruby code. They are clone-then-keep: nothing is applied unless the answer
# exists and the catalog is still unfrozen and unchanged.
class AnswersTest < Minitest::Test
  include CatalogHelpers

  # Runs the block with `klass.new` replaced by `body`, which gets the
  # original `new` and the arguments.
  def replacing_new(klass, body)
    singleton = klass.singleton_class
    singleton.alias_method(:__original_new, :new)
    singleton.remove_method(:new)
    original = klass.method(:__original_new)
    klass.define_singleton_method(:new) { |*args, **kw| body.call(original, *args, **kw) }
    yield
  ensure
    singleton.remove_method(:new)
    singleton.alias_method(:new, :__original_new)
    singleton.remove_method(:__original_new)
  end

  def raising = ->(*) { raise IOError, 'boom' }

  def freezing(catalog) = ->(original, *args, **kw) { catalog.freeze && original.call(*args, **kw) }

  def test_a_raising_answer_applies_nothing
    catalog = Catalog.new
    sunday = full(catalog, 1)
    before = state(catalog, sunday.id)
    replacing_new(Pending, raising) { assert_raises(IOError) { catalog.begin(sunday.id) } }
    assert_equal before, state(catalog, sunday.id)
    attempt = catalog.begin(sunday.id)
    before = state(catalog, sunday.id)
    replacing_new(Committed, raising) { assert_raises(IOError) { catalog.commit(attempt, 2) } }
    assert_equal before, state(catalog, sunday.id)
    assert_instance_of Committed, catalog.commit(attempt, 2)
  end

  def test_a_reentrant_begin_raises_and_applies_nothing
    catalog = Catalog.new
    inner = entered = nil
    reenter = lambda do |original, *args, **kw|
      unless entered # the nested call builds its own answer normally
        entered = true
        inner = catalog.begin(nil)
      end
      original.call(*args, **kw)
    end
    replacing_new(Pending, reenter) { assert_raises(RuntimeError) { catalog.begin(nil) } }
    # Only the nested begin stands: one id issued, its attempt open.
    assert_equal [1, inner], [catalog.issued, catalog.pending]
  end

  def test_a_reentrant_commit_raises_and_applies_nothing
    catalog = Catalog.new
    attempt = catalog.begin(nil)
    reenter = lambda do |original, *args, **kw|
      catalog.abort(attempt)
      original.call(*args, **kw)
    end
    replacing_new(Committed, reenter) { assert_raises(RuntimeError) { catalog.commit(attempt, 1) } }
    assert_nil catalog.latest
    assert_nil catalog.pending
    assert_equal :already_aborted, catalog.abort(attempt)
  end

  def test_an_answer_that_freezes_the_catalog_applies_nothing
    catalog = Catalog.new
    before = state(catalog)
    replacing_new(Pending, freezing(catalog)) { assert_raises(FrozenError) { catalog.begin(nil) } }
    assert_equal before, state(catalog)

    catalog = Catalog.new
    attempt = catalog.begin(nil)
    before = state(catalog)
    replacing_new(Manifest, freezing(catalog)) { assert_raises(FrozenError) { catalog.commit(attempt, 1) } }
    assert_equal before, state(catalog)
    assert_predicate catalog, :frozen?
  end

  # The classes are resolved once, when the extension loads.
  def test_classes_are_resolved_once_at_init
    catalog = Catalog.new
    pending = SnapshotMachines.send(:remove_const, :Pending)
    SnapshotMachines.const_set(:Pending, String)
    GC.start
    attempt = catalog.begin(nil)
  ensure
    SnapshotMachines.send(:remove_const, :Pending)
    SnapshotMachines.const_set(:Pending, pending)
    assert_instance_of Pending, attempt
    assert_instance_of Committed, catalog.commit(attempt, 1)
  end
end
