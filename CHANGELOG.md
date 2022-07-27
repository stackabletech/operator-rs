# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [0.23.0] - 2022-07-26

### Added

- Add `AuthenticationClass::resolve` helper function ([#432]).

### Changed

- BREAKING:kube `0.73.1` -> `0.74.0` ([#440]). Deprecate `ResourceExt::name` in favour of safe `name_*` alternatives. [kube-#945]
- `ContainerBuilder::new` validates container name to be RFC 1035-compliant([#292]).

[#432]: https://github.com/stackabletech/operator-rs/pull/432
[#440]: https://github.com/stackabletech/operator-rs/pull/440
[kube-#945]: https://github.com/kube-rs/kube-rs/pull/945

## [0.22.0] - 2022-07-05

### Added

- `startup_probe` added to `ContainerBuilder` ([#430]).

### Changed

- BREAKING: Bump to k8s 1.24 and kube 0.73.1 ([#408]).

### Fixed

- Correctly propagate storage class in `PVCConfig::build_pvc()` ([#412]).

[#408]: https://github.com/stackabletech/operator-rs/pull/408
[#412]: https://github.com/stackabletech/operator-rs/pull/412
[#430]: https://github.com/stackabletech/operator-rs/pull/430

## [0.21.1] - 2022-05-22

### Added

- `scale_to` and `to_java_heap_value` in `Memory` to scale units up or down ([#407]).

### Changed

- Visibility of `Memory` in `memory.rs` to private ([#407]).

[#407]: https://github.com/stackabletech/operator-rs/pull/407

## [0.21.0] - 2022-05-16

### Changed

- `impl Into<Resourcerequirements> for Resources` set's fields to `None` instead of `Some(<empty map>)` when nothing is defined. ([#398]).
- BREAKING: Change credentials of `S3ConnectionSpec` to use the common `SecretClassVolume` struct ([#405]).

[#398]: https://github.com/stackabletech/operator-rs/pull/398
[#405]: https://github.com/stackabletech/operator-rs/pull/405

## [0.20.0] - 2022-05-13

### Added

- Added `config::merge::chainable_merge()` ([#397]).
- `SecretClassVolume` and `SecretOperatorVolumeSourceBuilder` now support secret-aware pod scheduling ([#396], [secret-#125]).
- New `memory` module ([#400]).
- `S3AccessStyle` enum added to `commons::s3::S3ConnectionSpec` ([#401])

### Changed

- BREAKING: `SecretClassVolume::to_csi_volume` renamed to `to_ephemeral_volume` and now returns `EphemeralVolumeSource` ([#396]).
- BREAKING: `SecretOperatorVolumeSourceBuilder` now returns `EphemeralVolumeSource` ([#396]).
- BREAKING: Secret-Operator-related features now require Secret-Operator 0.4.0 ([#396]).
- BREAKING: Memory and CPU resource definitions use quantity instead of String ([#402])

[#396]: https://github.com/stackabletech/operator-rs/pull/396
[#397]: https://github.com/stackabletech/operator-rs/pull/397
[#400]: https://github.com/stackabletech/operator-rs/pull/400
[#401]: https://github.com/stackabletech/operator-rs/pull/401
[#402]: https://github.com/stackabletech/operator-rs/pull/402
[secret-#125]: https://github.com/stackabletech/secret-operator/pull/125

## [0.19.0] - 2022-05-05

### Changed

- BREAKING: Removed `commons::s3::S3ConnectionImplementation`. `commons::s3::InlinedBucketSpec::endpoint()` doesn't take arguments since the protocol decision is now based on the existance of TLS configuration ([#390]).
- BREAKING: Changes to resource requirements structs to enable deep merging ([#392])
  - Changed fields in `Resources` to no longer be optional
  - Changed atomic fields in `MemoryLimits`, `JvmHeapLimits`, `CpuLimits` and `PvcConfig` to be optional
- BREAKING: Removed `commons::tls::TlsMutualVerification` ([#394](https://github.com/stackabletech/operator-rs/issues/394)).

[#390]: https://github.com/stackabletech/operator-rs/issues/390
[#392]: https://github.com/stackabletech/operator-rs/pull/392

## [0.18.0] - 2022-05-04

### Added

- Typed `Merge` trait ([#368]).
- New commons::s3 module with common S3 connection structs ([#377]).
- New `TlsAuthenticationProvider` for `AuthenticationClass` ([#387]).

[#368]: https://github.com/stackabletech/operator-rs/pull/368
[#377]: https://github.com/stackabletech/operator-rs/issues/377
[#387]: https://github.com/stackabletech/operator-rs/pull/387

## [0.17.0] - 2022-04-14

### Changed

- product-config 0.3.1 -> 0.4.0 ([#373])
- kube 0.70.0 -> 0.71.0 ([#372])

[#372]: https://github.com/stackabletech/operator-rs/pull/372
[#373]: https://github.com/stackabletech/operator-rs/pull/373

## [0.16.0] - 2022-04-11

### Added

- Export logs to Jaeger ([#360]).
- Added common datastructures shared between all operators like `Tls` oder `AuthenticationClass` ([#366]).
- Added helpers for env variables from Secrets or ConfigMaps ([#370]).

### Changed

- BREAKING: `initialize_logging` now takes an app name and tracing target ([#360]).
- BREAKING: Move opa struct to commons ([#369]).

[#360]: https://github.com/stackabletech/operator-rs/pull/360
[#366]: https://github.com/stackabletech/operator-rs/pull/366
[#369]: https://github.com/stackabletech/operator-rs/pull/369
[#370]: https://github.com/stackabletech/operator-rs/pull/370

## [0.15.0] - 2022-03-21

### Added

- Common `OpaConfig` to specify a config map and package name ([#357]).

### Changed

- Split up the builder module into submodules. This is not breaking yet due to reexports. Deprecation warning has been added for `operator-rs` `0.15.0` ([#348]).
- Update to `kube` `0.70.0` ([Release Notes](https://github.com/kube-rs/kube-rs/releases/tag/0.70.0)). The signature and the Ok action in reconcile fns has been simplified slightly. Because of this the signature of `report_controller_reconciled` had to be changed slightly ([#359]).

[#348]: https://github.com/stackabletech/operator-rs/pull/348
[#357]: https://github.com/stackabletech/operator-rs/pull/357

## [0.14.1] - 2022-03-15

### Changed

- product-config 0.3.0 -> 0.3.1 ([#346])

[#346]: https://github.com/stackabletech/operator-rs/pull/346

## [0.14.0] - 2022-03-08

### Added

- Builder for CSI and Secret Operator volumes ([#342], [#344])

### Fixed

- Truncate k8s event strings correctly, when required ([#337]).

[#337]: https://github.com/stackabletech/operator-rs/pull/337
[#342]: https://github.com/stackabletech/operator-rs/pull/342
[#344]: https://github.com/stackabletech/operator-rs/pull/344

## [0.13.0] - 2022-02-23

### Added

- BREAKING: Added CLI `watch_namespace` parameter to ProductOperatorRun in
  preparation for operators watching a single namespace ([#332], [#333]).
- More builder functionality ([#331])
  - builder for `SecurityContext` objects
  - add `EnvVar`s from field refs
  - set `serviceServiceAccountName` in pod templates

### Changed

- Build against Kubernetes 1.23 ([#330]).

[#330]: https://github.com/stackabletech/operator-rs/pull/330
[#331]: https://github.com/stackabletech/operator-rs/pull/331
[#332]: https://github.com/stackabletech/operator-rs/pull/332
[#333]: https://github.com/stackabletech/operator-rs/pull/333

## [0.12.0] - 2022-02-18

### Changed

- Reported K8s events are now limited to 1024 bytes ([#327]).

### Removed

- `Client::set_condition` ([#326]).
- `Error` variants that are no longer used ([#326]).

[#326]: https://github.com/stackabletech/operator-rs/pull/326
[#327]: https://github.com/stackabletech/operator-rs/pull/327

## [0.11.0] - 2022-02-17

### Added

- Infrastructure for logging errors as K8s events ([#322]).

### Changed

- BREAKING: kube 0.68 -> 0.69.1 ([#319, [#322]]).

### Removed

- Chrono's time 0.1 compatibility ([#310]).
- Deprecated pre-rework utilities ([#320]).

[#310]: https://github.com/stackabletech/operator-rs/pull/310
[#319]: https://github.com/stackabletech/operator-rs/pull/319
[#320]: https://github.com/stackabletech/operator-rs/pull/320
[#322]: https://github.com/stackabletech/operator-rs/pull/322

## [0.10.0] - 2022-02-04

### Added

- Unified `ClusterRef` type for referring to cluster objects ([#307]).

### Changed

- BREAKING: kube 0.66 -> 0.68 ([#303]).
- BREAKING: k8s-openapi 0.13 -> 0.14 ([#303]).

### Removed

- Auto-generated service link environment variables for built pods ([#305]).

[#303]: https://github.com/stackabletech/operator-rs/pull/303
[#305]: https://github.com/stackabletech/operator-rs/pull/305
[#307]: https://github.com/stackabletech/operator-rs/pull/307

## [0.9.0] - 2022-01-27

### Changed

- Fixed `Client::apply_patch_status` always failing ([#300]).

[#300]: https://github.com/stackabletech/operator-rs/pull/300

## [0.8.0] - 2022-01-17

### Added

- Allow adding custom CLI arguments to `run` subcommand ([#291]).

### Changed

- BREAKING: clap 2.33.3 -> 3.0.4 ([#289]).
- BREAKING: kube 0.65 -> 0.66 ([#293]).
- BREAKING: `cli::Command::Run` now just wraps `cli::ProductOperatorRun` rather than defining the struct inline ([#291]).

[#289]: https://github.com/stackabletech/operator-rs/pull/289
[#291]: https://github.com/stackabletech/operator-rs/pull/291
[#293]: https://github.com/stackabletech/operator-rs/pull/293

## [0.7.0] - 2021-12-22

### Changed

- BREAKING: Introduced proper (Result) error handling for `transform_all_roles_to_config` ([#282]).
- BREAKING: `Configuration::compute_*` are now invoked even when `config` field is not provided on `Role`/`RoleGroup` ([#282]).
  - `CommonConfiguration::config` is no longer `Option`al
  - `Role::config` is no longer `Option`al
  - `RoleGroup::config` is no longer `Option`al
- Fixed `cli::Command` including developer-facing docs in `--help` output ([#283])

[#282]: https://github.com/stackabletech/operator-rs/pull/282
[#283]: https://github.com/stackabletech/operator-rs/pull/283

## [0.6.0] - 2021-12-13

### Changed

- BREAKING: kube-rs 0.63.1 -> 0.65.0 ([#277])
- strum 0.22.0 -> 0.23.0 ([#277])
- Undeprecated `CustomResourceExt` ([#279])

[#277]: https://github.com/stackabletech/operator-rs/pull/277
[#279]: https://github.com/stackabletech/operator-rs/pull/279

## [0.5.0] - 2021-12-09

### Added

- `build_template` to `PodBuilder` ([#259]).
- `readiness_probe` and `liveness_probe` to `ContainerBuilder` ([#259]).
- `role_group_selector_labels` to `labels` ([#261]).
- `role_selector_labels` to `labels` ([#270]).
- `Box<T: Configurable>` is now `Configurable` ([#262]).
- `node_selector` to `PodBuilder` ([#267]).
- `role_utils::RoleGroupRef` ([#272]).
- Add support for managing CLI commands via `StructOpt` ([#273]).

### Changed

- BREAKING: `ObjectMetaBuilder::build` is no longer fallible ([#259]).
- BREAKING: `PodBuilder::metadata_builder` is no longer fallible ([#259]).
- `role_utils::transform_all_roles_to_config` now takes any `T: Configurable`, not just `Box<T>` ([#262]).
- BREAKING: Type-erasing `Role<T>` into `Role<Box<dyn Configurable>>` must now be done using `Role::erase` rather than `Role::into` ([#262]).
- BREAKING: Changed all `&Option<T>` into `Option<&T>`, some code will need to be rewritten to use `Option::as_ref` rather than `&foo` ([#263]).
- Promoted controller watch failures to WARN log level (from TRACE) ([#269]).

[#259]: https://github.com/stackabletech/operator-rs/pull/259
[#261]: https://github.com/stackabletech/operator-rs/pull/261
[#262]: https://github.com/stackabletech/operator-rs/pull/262
[#263]: https://github.com/stackabletech/operator-rs/pull/263
[#267]: https://github.com/stackabletech/operator-rs/pull/267
[#269]: https://github.com/stackabletech/operator-rs/pull/269
[#270]: https://github.com/stackabletech/operator-rs/pull/270
[#272]: https://github.com/stackabletech/operator-rs/pull/272
[#273]: https://github.com/stackabletech/operator-rs/pull/273

## [0.4.0] - 2021-11-05

### Added

- `VolumeBuilder` and `VolumeMountBuilder` ([#253]).
- `image_pull_policy` to `ContainerBuilder` ([#253]).
- `host_network` to `PodBuilder` ([#253]).

### Changed

- BREAKING: In builder: `add_stackable_agent_tolerations` to `add_tolerations` ([#255]).
- Generic `VALUE` paramters to `impl Into<_>` arguments for consistency ([#253]).

### Removed

- `krustlet.rs` ([#255]).
- `find_nodes_that_fit_selectors` no longer adds label `type=krustlet` to selector ([#255]).
- BREAKING: `configmaps` field from container builder ([#253]).
- BREAKING: Automatic `Volume` and `VolumeMount` creation from the `configmaps` field ([#253]).

[#255]: https://github.com/stackabletech/operator-rs/pull/255
[#253]: https://github.com/stackabletech/operator-rs/pull/253

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
