# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [0.1.1] - 2024-07-09

### Added

- Add support for versioned enums ([#813]).
- Add collision check for renamed fields ([#804]).
- Add auto-generated `From<OLD> for NEW` implementations ([#790]).

### Changed

- Change from derive macro to attribute macro to be able to generate code
  _in place_ instead of _appending_ new code ([#793]).
- Improve action chain generation ([#784]).
- Bump rust-toolchain to 1.79.0 ([#822])

[#784]: https://github.com/stackabletech/operator-rs/pull/784
[#790]: https://github.com/stackabletech/operator-rs/pull/790
[#793]: https://github.com/stackabletech/operator-rs/pull/793
[#804]: https://github.com/stackabletech/operator-rs/pull/804
[#813]: https://github.com/stackabletech/operator-rs/pull/813
[#822]: https://github.com/stackabletech/operator-rs/pull/822

## [0.1.0] - 2024-05-08

### Changed

- Bump Rust dependencies and GitHub Actions ([#782]).

[#782]: https://github.com/stackabletech/operator-rs/pull/782
