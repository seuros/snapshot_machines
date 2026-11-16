# Changelog

## [0.3.4](https://github.com/seuros/snapshot_machines/compare/snapshot_machines/v0.3.3...snapshot_machines/v0.3.4) (2026-11-16)


### Bug Fixes

* fix wrapped classes at load so building a value runs no Ruby ([119e9e1](https://github.com/seuros/snapshot_machines/commit/119e9e16094df18ac1dbae0519c4040e5edbcd49))

## [0.3.3](https://github.com/seuros/snapshot_machines/compare/snapshot_machines/v0.3.2...snapshot_machines/v0.3.3) (2026-11-16)


### Bug Fixes

* read SnapshotId#scope natively and drop catalog indexing ([4cf221f](https://github.com/seuros/snapshot_machines/commit/4cf221feeeea681939a817187c901e2596f9d862))

## [0.3.2](https://github.com/seuros/snapshot_machines/compare/snapshot_machines/v0.3.1...snapshot_machines/v0.3.2) (2026-11-14)


### Bug Fixes

* **deps:** require matryona-machines &gt;= 0.4 ([5097264](https://github.com/seuros/snapshot_machines/commit/50972648cf3a83e2959cbac99f9443d3560b6dcb))

## [0.3.1](https://github.com/seuros/snapshot_machines/compare/snapshot_machines/v0.3.0...snapshot_machines/v0.3.1) (2026-11-14)


### Bug Fixes

* **deps:** require matryona-machines &gt;= 0.3 and read bases with optional_fields ([8eefed6](https://github.com/seuros/snapshot_machines/commit/8eefed6a45d03f905334f70513dd121134803c6c))

## [0.3.0](https://github.com/seuros/snapshot_machines/compare/snapshot_machines/v0.2.0...snapshot_machines/v0.3.0) (2026-11-14)


### ⚠ BREAKING CHANGES

* adopt matryona-machines 0.2 (exact_fields, exact_array, symbols!, frozen-first)

### Features

* adopt matryona-machines 0.2 (exact_fields, exact_array, symbols!, frozen-first) ([3db1cfb](https://github.com/seuros/snapshot_machines/commit/3db1cfb3ad41909d03075942fb5dbfe0cdc795af))

## [0.2.0](https://github.com/seuros/snapshot_machines/compare/snapshot_machines/v0.1.5...snapshot_machines/v0.2.0) (2026-11-14)


### ⚠ BREAKING CHANGES

* scoped snapshot ids, Catalog::restored rebuilds the chain, private Manifest fields, exact-class boundary, clone-then-keep begin/commit

### Features

* scoped snapshot ids, Catalog::restored rebuilds the chain, private Manifest fields, exact-class boundary, clone-then-keep begin/commit ([21d16cd](https://github.com/seuros/snapshot_machines/commit/21d16cde3829ab1a389295c6e4d3b1e04faeb2a2))

## [0.1.5](https://github.com/seuros/snapshot_machines/compare/snapshot_machines/v0.1.4...snapshot_machines/v0.1.5) (2026-11-06)


### Bug Fixes

* extract catalog ownership check for commit/abort ([f501351](https://github.com/seuros/snapshot_machines/commit/f5013515b918dfb5627939ae2d06b191298b5a85))

## [0.1.4](https://github.com/seuros/snapshot_machines/compare/snapshot_machines/v0.1.3...snapshot_machines/v0.1.4) (2026-11-06)


### Bug Fixes

* crates.io keywords and categories for the core ([f544853](https://github.com/seuros/snapshot_machines/commit/f544853222f20d5e56b58199641258a676455f61))

## [0.1.3](https://github.com/seuros/snapshot_machines/compare/snapshot_machines/v0.1.2...snapshot_machines/v0.1.3) (2026-11-06)


### Bug Fixes

* keep test sources out of the packaged gem and crate ([002251a](https://github.com/seuros/snapshot_machines/commit/002251a711dd02f261ed4be6f255238fae2ef3ed))

## [0.1.2](https://github.com/seuros/snapshot_machines/compare/snapshot_machines/v0.1.1...snapshot_machines/v0.1.2) (2026-11-06)


### Bug Fixes

* cross builds no longer compile for the host ([e771bd5](https://github.com/seuros/snapshot_machines/commit/e771bd5f973692eef8035958143e29dc442237e7))

## [0.1.1](https://github.com/seuros/snapshot_machines/compare/snapshot_machines-v0.1.0...snapshot_machines/v0.1.1) (2026-11-06)


### Features

* initial extraction ([691a57e](https://github.com/seuros/snapshot_machines/commit/691a57e297015cf920bd3afc3ac9309c959c3af6))
