# frozen_string_literal: true
require 'bundler/gem_tasks'
require 'minitest/test_task'
require 'shellwords'
require 'rb_sys/extensiontask'
Minitest::TestTask.create(:minitest) do |task|
  task.libs << 'test' << 'lib'
  task.test_globs = ['test/**/*_test.rb']
end
GEMSPEC = Gem::Specification.load('snapshot_machines.gemspec')
RbSys::ExtensionTask.new('snapshot_machines_native', GEMSPEC) do |extension|
  extension.lib_dir = 'lib/snapshot_machines_native'
  extension.tmp_dir = 'tmp/rb_sys'
end
desc 'Compile and test the native Ruby boundary'
task test: :compile do
  sh 'padded_cell', *Shellwords.split(ENV.fetch('TESTOPTS', ''))
end
desc 'Validate type signatures'
task :rbs do
  sh 'rbs', '-I', 'sig', 'validate'
end
desc 'Test allocation-free core'
task :cargo do
  sh 'cargo', 'test', '-p', 'snapshot-machines'
end
namespace :native do
  task build: ['rb_sys:env:release', 'compile']
end
desc 'Lint the Rust workspace strictly'
task :clippy do
  sh 'cargo', 'clippy', '--workspace', '--all-targets', '--', '-D', 'warnings'
end
task default: %i[test cargo rbs]

Dir[File.join(__dir__, 'tasks/*.rake')].sort.each { |f| load f }
