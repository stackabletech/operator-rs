# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- Add new `from_type` parameter to `changed()` action ([#844]).
- Pass through container and item attributes (including doc-comments). Add
  attribute for version specific docs ([#847]).
- Forward container visibility to generated modules ([#850]).

### Changed

- BREAKING: Rename `renamed()` action to `changed()` and renamed `from`
  parameter to `from_name` ([#844]).

### Fixed

- Report variant rename validation error at the correct span and trim underscores
  from variants not using PascalCase (#[842]).

[#842]: https://github.com/stackabletech/operator-rs/pull/842
[#844]: https://github.com/stackabletech/operator-rs/pull/844
[#847]: https://github.com/stackabletech/operator-rs/pull/847
[#850]: https://github.com/stackabletech/operator-rs/pull/850

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
