# frozen_string_literal: true

require_relative 'test_helper'
require 'open3'
require 'rbconfig'

# The class of the value the extension wraps (Catalog) is fixed when it
# loads. Nothing a caller does later, redefining the constant or
# `Object.const_get`, reaches a catalog being built. Each case runs in a fresh
# process: the class is fixed once per process, at load.
class ClassLookupTest < Minitest::Test
  LIB = File.expand_path('../lib', __dir__)

  PRELUDE = <<~RUBY
    require 'snapshot_machines'
    S = SnapshotMachines
    # One full manifest committed: the catalog works.
    def used(catalog) = catalog.commit(catalog.begin(nil), 42).manifest.checksum
  RUBY

  # Runs `body` after PRELUDE in a new Ruby; answers its last line of output.
  def isolated(body)
    out, err, status = Open3.capture3(RbConfig.ruby, '-I', LIB, '-e', PRELUDE + body)
    assert status.success?, "exited #{status.exitstatus}: #{err}"
    out.lines.last&.chomp
  end

  # Bug: a Catalog constant redefined before the first catalog made it an
  # instance of the new class, without any of the catalog's methods.
  def test_a_redefined_catalog_constant_does_not_change_the_catalog_class
    assert_equal 'true 42 true', isolated(<<~RUBY)
      original = S::Catalog
      S.send(:remove_const, :Catalog)
      S.const_set(:Catalog, Class.new)
      catalog = original.new
      puts [catalog.instance_of?(original), used(catalog),
            original.with_scope(3).instance_of?(original)].join(' ')
    RUBY
  end

  # Bug: building the first catalog ran `Object.const_get`, a caller's Ruby
  # code in the middle of a native call.
  def test_building_a_catalog_runs_no_const_get
    assert_equal '0 Catalog Catalog Catalog', isolated(<<~RUBY)
      hits = 0
      Object.define_singleton_method(:const_get) do |*args|
        hits += 1
        super(*args)
      end
      catalogs = [S::Catalog.new, S::Catalog.with_scope(3), S::Catalog.restored(0, 0, [])]
      puts [hits, *catalogs.map { |catalog| catalog.class.name.split('::').last }].join(' ')
    RUBY
  end

  # Bug: a redefined `Object.const_get` that raised hit an `unwrap` in the
  # extension: an uncatchable fatal, and the process exited.
  def test_a_raising_const_get_does_not_kill_the_process
    assert_equal '42', isolated(<<~RUBY)
      Object.define_singleton_method(:const_get) { |*| raise IOError, 'lookup ran mid-call' }
      puts used(S::Catalog.new)
    RUBY
  end
end
