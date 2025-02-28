# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- Add support for re-emitting and merging modules defined in versioned modules ([#971]).
- Add basic support for generic types in struct and enum definitions ([#969]).

### Changed

- BREAKING: Move `preserve_module` option into `options` to unify option interface ([#961]).

[#961]: https://github.com/stackabletech/operator-rs/pull/961
[#969]: https://github.com/stackabletech/operator-rs/pull/969
[#971]: https://github.com/stackabletech/operator-rs/pull/971

## [0.5.1] - 2025-02-14

### Added

- Add support for multiple k8s `shortname` arguments ([#958]).

[#958]: https://github.com/stackabletech/operator-rs/pull/958

## [0.5.0] - 2024-12-03

### Added

- Use visibility of container definition or module for generated CRD enum ([#923]).
- Add support to apply the `#[versioned()]` macro to modules to version all contained items at
  once ([#891]).
- Add support for passing a `status`, `crates`, and `shortname` arguments through to the `#[kube]`
  derive attribute ([#914]).
- Add support for overriding `kube::core` and `k8s_openapi` in generated code ([#914]).

### Removed

- BREAKING: Remove {write,print}_merged_crd functions ([#924]).
- BREAKING: Remove the `CustomResource` derive ([#914]).

### Changed

- Simplify crate override handling and generation ([#919]).
- Bump Rust to 1.82.0 ([#891]).
- Refactor the Override type ([#922]).

### Fixed

- Emit correct enum ident based on kube/k8s kind argument ([#920]).
- Generate Kubernetes code independent of container order ([#913]).
- Correctly emit Kubernetes code when macro is used on modules ([#912]).
- Use `.into()` on all field conversions ([#925]).
- Remove invalid type comparison on field conversion because the semantics are unknown ([#925]).
- Check whether to skip all from impls when versioning a module ([#926]).

[#891]: https://github.com/stackabletech/operator-rs/pull/891
[#912]: https://github.com/stackabletech/operator-rs/pull/912
[#913]: https://github.com/stackabletech/operator-rs/pull/913
[#914]: https://github.com/stackabletech/operator-rs/pull/914
[#919]: https://github.com/stackabletech/operator-rs/pull/919
[#920]: https://github.com/stackabletech/operator-rs/pull/920
[#922]: https://github.com/stackabletech/operator-rs/pull/922
[#923]: https://github.com/stackabletech/operator-rs/pull/923
[#924]: https://github.com/stackabletech/operator-rs/pull/924
[#925]: https://github.com/stackabletech/operator-rs/pull/925
[#926]: https://github.com/stackabletech/operator-rs/pull/926

## [0.4.1] - 2024-10-23

### Added

- Add basic handling for enum variants with data ([#892]).

### Fixed

- Accept a wider variety of formatting styles in the macro testing regex ([#892]).

[#892]: https://github.com/stackabletech/operator-rs/pull/892

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
