# frozen_string_literal: true

require 'minitest/autorun'
require 'snapshot_machines'

module CatalogHelpers
  include SnapshotMachines

  # Everything a catalog reveals: equal before and after a refusal means
  # nothing changed.
  def state(catalog, *targets)
    [catalog.scope, catalog.issued, catalog.pending, catalog.latest, targets.map { catalog.restore_plan(_1) }]
  end

  def assert_refused(symbol, catalog, *targets)
    before = state(catalog, *targets)
    assert_equal symbol, yield
    assert_equal before, state(catalog, *targets)
  end

  def full(catalog, checksum)
    catalog.commit(catalog.begin(nil), checksum).manifest
  end
end
