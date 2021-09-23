# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Changed
- Bugfix: pod id generation from pods should match the output of `generate_ids` when pods have no id label. ([#222])
- Bugfix: When scheduling a pod, `GroupAntiAffinityStrategy` should not skip nodes that are mapped by other pods from different role+group. ([#222])

### Added
- `scheduler::PodToNodeMapping::try_from_pods_and_id_label` as a replacement for the deleted `scheduler::PodToNodeMapping::from` function. Relevant for the zookeeper-operator. ([#222])
- `scheduler::PodToNodeMapping::try_from_pods` to support scheduling pods without an explicit `id` label. ([#222])

### Removed
 `scheduler::PodToNodeMapping::from` ([#222])

[#222]: https://github.com/stackabletech/operator-rs/pull/222

## [0.2.2] - 2021-09-21


### Changed

- `kube-rs`: `0.59` → `0.60` ([#217]).
- BREAKING: `kube-rs`: `0.58` → `0.59` ([#186]).

[#217]: https://github.com/stackabletech/operator-rs/pull/217
[#186]: https://github.com/stackabletech/operator-rs/pull/186

## [0.2.1] - 2021-09-20

### Added
- Getter for `scheduler::PodIdentity` fields ([#215]).

[#215]: https://github.com/stackabletech/operator-rs/pull/215

## [0.2.0] - 2021-09-17


### Added
- Extracted the versioning support for up and downgrades from operators ([#211]).
- Added traits to access generic operator versions ([#211]).
- Added init_status method that uses the status default ([#211]).
- Implement StickyScheduler with two pod placement strategies and history stored as K8S status field. ([#210])

### Changed
- `BREAKING`: Changed `Conditions` trait return value to not optional ([#211]). 

[#211]: https://github.com/stackabletech/operator-rs/pull/211
[#210]: https://github.com/stackabletech/operator-rs/pull/210

## 0.1.0 - 2021-09-01

### Added

- Initial release
