# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Changed
- BREAKING: In builder: `add_stackable_agent_tolerations` to `add_tolerations` ([#255]).

### Removed
- `krustlet.rs` ([#255]).
- `find_nodes_that_fit_selectors` no longer adds label `type=krustlet` to selector ([#255]).

[#255]: https://github.com/stackabletech/operator-rs/pull/255

## [0.3.0] - 2021-10-27

### Fixed
- Bugfix: when scheduling a pod, `GroupAntiAffinityStrategy` should not skip nodes that are mapped by other pods from different role+group. ([#222])
- Bugfix: annotate `conditions` as map-list ([#226])
  - Requires manual action: add `#[schemars(schema_with = "stackable_operator::conditions::conditions_schema")]` annotation to `conditions` field in your status struct
- BREAKING: `Client::apply_patch` and `Client::apply_patch_status` now take a `context` argument that scopes their fieldManager ([#225])
- Bugfix: `Client::set_condition` now scopes its fieldManager to the condition being applied ([#225])
- Bugfix: removed duplicate object identity from reconciler. ([#228])
- Bugfix: added proper error handling for versioning. If versions are not supported or invalid an error is thrown which should stop further reconciliation ([#236]).

### Added
- `command.rs` module to handle common command operations ([#184]).
- Traits for command handling ([#184]):
  - `HasCurrentCommand` to manipulate the current_command in the status
  - `HasClusterExecutionStatus` to access cluster_execution_status in the status
  - `HasRoleRestartOrder` to determine the restart order of different roles
  - `HasCommands` to provide all supported commands like Restart, Start, Stop ...
  - `CanBeRolling` to perform a rolling restart
  - `HasRoles` to run a command only on a subset of roles
- Enum `ClusterExecutionStatus` to signal that the cluster is running or stopped ([#184]).
- Default implementations for Restart, Start and Stop commands ([#184]).
- `identity.rs` a new module split out of `scheduler.rs` that bundles code for pod and node id management.
- `identity::PodIdentityFactory` trait and one implementation called `identity::LabeledPodIdentityFactory`.
- `controller.rs` - Configurable requeue timeout

### Removed
- `reconcile::create_config_maps` which is obsolete and replaced by `configmap::create_config_maps` ([#184])
- BREAKING: `scheduler::PodToNodeMapping::from` ([#222]).
- Reexport `kube`, `k8s-openapi`, `schemars` ([#247])

[#184]: https://github.com/stackabletech/operator-rs/pull/184
[#222]: https://github.com/stackabletech/operator-rs/pull/222
[#226]: https://github.com/stackabletech/operator-rs/pull/226
[#225]: https://github.com/stackabletech/operator-rs/pull/225
[#228]: https://github.com/stackabletech/operator-rs/pull/228
[#236]: https://github.com/stackabletech/operator-rs/pull/236
[#247]: https://github.com/stackabletech/operator-rs/pull/247

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
