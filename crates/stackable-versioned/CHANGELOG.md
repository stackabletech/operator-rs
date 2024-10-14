# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [0.4.0] - 2024-10-14

### Added

- Add YAML serialization for merged CRD schema. The schema can now be printed to stdout or written
  to file ([#884]).
- Add snapshot tests to verify generated code matches expected output ([#881]).

[#881]: https://github.com/stackabletech/operator-rs/pull/881
[#884]: https://github.com/stackabletech/operator-rs/pull/884

## [0.3.0] - 2024-09-26

### Added

- Add forwarding of `singular`, `plural`, and `namespaced` arguments in `k8s()`
  ([#873]).
- Generate a `Version` enum containing all declared versions as variants
  ([#872]).

### Changed

- BREAKING: The `merged_crd` function now accepts `Self` instead of a dedicated
  `Version` enum ([#875]).
- The `merged_crd` associated function now takes `Version` instead of `&str` as
  input ([#872]).

[#872]: https://github.com/stackabletech/operator-rs/pull/872
[#873]: https://github.com/stackabletech/operator-rs/pull/873
[#875]: https://github.com/stackabletech/operator-rs/pull/875

## [0.2.0] - 2024-09-19

### Added

- Add `from_name` parameter validation ([#865]).
- Add new `from_type` parameter to `changed()` action ([#844]).
- Pass through container and item attributes (including doc-comments). Add
  attribute for version specific docs ([#847]).
- Forward container visibility to generated modules ([#850]).
- Add support for Kubernetes-specific features ([#857]).
- Add `use super::*` to version modules to be able to use imported types
  ([#859]).

### Changed

- BREAKING: Rename `renamed()` action to `changed()` and renamed `from`
  parameter to `from_name` ([#844]).
- Bump syn to 2.0.77 ([#857]).

### Fixed

- Report variant rename validation error at the correct span and trim underscores
  from variants not using PascalCase ([#842]).
- Emit correct struct field types for fields with no changes (NoChange) ([#860]).

[#842]: https://github.com/stackabletech/operator-rs/pull/842
[#844]: https://github.com/stackabletech/operator-rs/pull/844
[#847]: https://github.com/stackabletech/operator-rs/pull/847
[#850]: https://github.com/stackabletech/operator-rs/pull/850
[#857]: https://github.com/stackabletech/operator-rs/pull/857
[#859]: https://github.com/stackabletech/operator-rs/pull/859
[#860]: https://github.com/stackabletech/operator-rs/pull/860
[#865]: https://github.com/stackabletech/operator-rs/pull/865

## [0.1.1] - 2024-07-10

### Added

- Add support for versioned enums ([#813]).
- Add collision check for renamed fields ([#804]).
- Add auto-generated `From<OLD> for NEW` implementations ([#790]).

### Changed

- Remove duplicated code and unified struct/enum and field/variant code ([#820]).
- Change from derive macro to attribute macro to be able to generate code
  _in place_ instead of _appending_ new code ([#793]).
- Improve action chain generation ([#784]).
- Bump rust-toolchain to 1.79.0 ([#822]).

[#784]: https://github.com/stackabletech/operator-rs/pull/784
[#790]: https://github.com/stackabletech/operator-rs/pull/790
[#793]: https://github.com/stackabletech/operator-rs/pull/793
[#804]: https://github.com/stackabletech/operator-rs/pull/804
[#813]: https://github.com/stackabletech/operator-rs/pull/813
[#820]: https://github.com/stackabletech/operator-rs/pull/820
[#822]: https://github.com/stackabletech/operator-rs/pull/822

## [0.1.0] - 2024-05-08

### Changed

- Bump Rust dependencies and GitHub Actions ([#782]).

[#782]: https://github.com/stackabletech/operator-rs/pull/782
