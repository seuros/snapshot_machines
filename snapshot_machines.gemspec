# frozen_string_literal: true
require_relative 'lib/snapshot_machines/version'
Gem::Specification.new do |spec|
  spec.name = 'snapshot_machines'
  spec.version = SnapshotMachines::VERSION
  spec.authors = ['Abdelkader Boudih']
  spec.email = ['oss@seuros.com']
  spec.summary = 'Bounded native snapshot manifest planning for Ruby'
  spec.description = 'An allocation-free Rust manifest model with a thin Ruby bridge. ' \
                     'No snapshot bytes or durable storage.'
  spec.homepage = 'https://github.com/seuros/snapshot_machines'
  spec.license = 'MIT'
  spec.required_ruby_version = '>= 4.0.0'
  spec.metadata['source_code_uri'] = spec.homepage
  spec.metadata['bug_tracker_uri'] = "#{spec.homepage}/issues"
  spec.metadata['rubygems_mfa_required'] = 'true'
  spec.metadata['cargo_crate_name'] = 'snapshot_machines_native'
  spec.metadata['cargo_manifest_path'] = 'ext/snapshot_machines_native/ffi/Cargo.toml'
  spec.files = Dir.glob(%w[
    lib/**/*.rb sig/**/*.rbs ext/**/*.{rb,rs,toml,md} Cargo.toml
    Cargo.lock README.md CHANGELOG.md LICENSE.txt
  ]).select { |path| File.file?(path) }
    .reject { |path| path.end_with?('/tests.rs') || path.include?('/tests/') }
  spec.require_paths = ['lib']
  spec.extensions = ['ext/snapshot_machines_native/extconf.rb']
  spec.add_dependency 'rb_sys', '~> 0.9', '>= 0.9.130'
end
