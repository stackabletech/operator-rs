# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [0.2.0] - 2025-05-26

### Changed

- Increased the default `--loop` interval from every minute to every 30 minutes ([#23]).
- Collect and output the open files limit ([#45]).

### Fixes

- Move the span inside the loop ([#46]).

[#23]: https://github.com/stackabletech/containerdebug/pull/23
[#45]: https://github.com/stackabletech/containerdebug/pull/45
[#46]: https://github.com/stackabletech/containerdebug/pull/46

## [0.1.1] - 2024-12-16

### Changed

- Downgraded DNS errors to warnings ([#17]).
- All output is now wrapped in a "containerdebug" span ([#18]).

### Fixes

- Reduced memory usage dramatically by limiting and caching fetched information ([#20]).

[#17]: https://github.com/stackabletech/containerdebug/pull/17
[#18]: https://github.com/stackabletech/containerdebug/pull/18
[#20]: https://github.com/stackabletech/containerdebug/pull/20

## [0.1.0] - 2024-12-09

### Added

- Initial release.
