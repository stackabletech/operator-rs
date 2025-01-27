# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Changed

- Deprecate `stackable_operator::logging::initialize_logging()`. It's recommended to use `stackable-telemetry` instead.

## [0.84.1] - 2025-01-22

### Fixed

- Remove `Merge` trait bound from `erase` and make `product_specific_common_config` public ([#946]).
- BREAKING: Revert the change of appending a dot to the default cluster domain to make it a FQDN, it is now `cluster.local` again. Users can instead explicitly opt-in to FQDNs via the ENV variable `KUBERNETES_CLUSTER_DOMAIN`. ([#947]).

[#946]: https://github.com/stackabletech/operator-rs/pull/946
[#947]: https://github.com/stackabletech/operator-rs/pull/947

## [0.84.0] - 2025-01-16

### Added

- BREAKING: Aggregate emitted Kubernetes events on the CustomResources thanks to the new
  [kube feature](https://github.com/kube-rs/controller-rs/pull/116). Instead of reporting the same
  event multiple times it now uses `EventSeries` to aggregate these events to single entry with an
  age like `3s (x11 over 53s)` ([#938]):
  - The `report_controller_error` function now needs to be async.
  - It now takes `Recorder` as a parameter instead of a `Client`.
  - The `Recorder` instance needs to be available across all `reconcile` invocations, to ensure
    aggregation works correctly.
  - The operator needs permission to `patch` events (previously only `create` was needed).
- Add `ProductSpecificCommonConfig`, so that product operators can have custom fields within `commonConfig`.
  Also add a `JavaCommonConfig`, which can be used by JVM-based tools to offer `jvmArgumentOverrides` with this mechanism ([#931])

### Changed

- BREAKING: Bump Rust dependencies to enable Kubernetes 1.32 (via `kube` 0.98.0 and `k8s-openapi` 0.23.0) ([#938]).
- BREAKING: Append a dot to the default cluster domain to make it a FQDN and allow FQDNs when validating a `DomainName` ([#939]).

[#931]: https://github.com/stackabletech/operator-rs/pull/931
[#938]: https://github.com/stackabletech/operator-rs/pull/938
[#939]: https://github.com/stackabletech/operator-rs/pull/939

## [0.83.0] - 2024-12-03

### Added

- Added cert lifetime setter to `SecretOperatorVolumeSourceBuilder` ([#915])

### Changed

- Replace unmaintained `derivative` crate with `educe` ([#907]).
- Bump dependencies, notably rustls 0.23.15 to 0.23.19 to fix [RUSTSEC-2024-0399] ([#917]).

[#907]: https://github.com/stackabletech/operator-rs/pull/907
[#915]: https://github.com/stackabletech/operator-rs/pull/915
[#917]: https://github.com/stackabletech/operator-rs/pull/917
[RUSTSEC-2024-0399]: https://rustsec.org/advisories/RUSTSEC-2024-0399

## [0.82.0] - 2024-11-23

### Fixed

- Fixed URL handling related to OIDC and `rootPath` with and without trailing slashes. Also added a bunch of tests ([#910]).

### Changed

- BREAKING: Made `DEFAULT_OIDC_WELLKNOWN_PATH` private. Use `AuthenticationProvider::well_known_config_url` instead ([#910]).
- BREAKING: Changed visibility of `commons::rbac::service_account_name` and `commons::rbac::role_binding_name` to
  private, as these functions should not be called directly by the operators. This is likely to result in naming conflicts
  as the result is completely dependent on what is passed to this function. Operators should instead rely on the roleBinding
  and serviceAccount objects created by `commons::rbac::build_rbac_resources` and retrieve the name from the returned
  objects if they need it ([#909]).
- Changed the names of the objects that are returned from `commons::rbac::build_rbac_resources` to not rely solely on the product
  they refer to (e.g. "nifi-rolebinding") but instead include the name of the resource to be unique per cluster
  (e.g. simple-nifi-rolebinding) ([#909]).

[#909]: https://github.com/stackabletech/operator-rs/pull/909
[#910]: https://github.com/stackabletech/operator-rs/pull/910

## [0.81.0] - 2024-11-05

### Added

- Add new `PreferredAddressType::HostnameConservative` ([#903]).

### Changed

- BREAKING: Split `ListenerClass.spec.preferred_address_type` into a new `PreferredAddressType` type. Use `resolve_preferred_address_type()` to access the `AddressType` as before ([#903]).

[#903]: https://github.com/stackabletech/operator-rs/pull/903

## [0.80.0] - 2024-10-23

### Changed

- BREAKING: Don't parse `/etc/resolv.conf` to auto-detect the Kubernetes cluster domain in case it is not explicitly configured.
  Instead the operator will default to `cluster.local`. We revert this now after some concerns where raised, we will
  create a follow-up decision instead addressing how we will continue with this ([#896]).
- Update Rust dependencies (Both `json-patch` and opentelemetry crates cannot be updated because of conflicts) ([#897]):
  - Bump `kube` to `0.96.0`,
  - `rstest` to `0.23.0` and
  - `tower-http` to `0.6.1`

### Fixed

- Fix Kubernetes cluster domain parsing from resolv.conf, e.g. on AWS EKS.
  We now only consider Kubernetes services domains instead of all domains (which could include non-Kubernetes domains) ([#895]).

[#895]: https://github.com/stackabletech/operator-rs/pull/895
[#896]: https://github.com/stackabletech/operator-rs/pull/896
[#897]: https://github.com/stackabletech/operator-rs/pull/897

## [0.79.0] - 2024-10-18

### Added

- Re-export the `YamlSchema` trait and the `stackable-shared` crate as the `shared` module ([#883]).
- BREAKING: Added `preferredAddressType` field to ListenerClass CRD ([#885]).
- BREAKING: The cluster domain (default: `cluster.local`) can now be configured in the individual
  operators via the ENV variable `KUBERNETES_CLUSTER_DOMAIN` or resolved automatically by parsing
  the `/etc/resolve.conf` file. This requires using `initialize_operator` instead of `create_client`
  in the `main.rs` of the individual operators ([#893]).

### Changed

- BREAKING: The `CustomResourceExt` trait is now re-exported from the `stackable-shared` crate. The
  trait functions use the same parameters but return a different error type ([#883]).
- BREAKING: `KeyValuePairs` (as well as `Labels`/`Annotations` via it) is now backed by a `BTreeMap`
  rather than a `BTreeSet` ([#888]).
  - The `Deref` impl now returns a `BTreeMap` instead.
  - `iter()` now clones the values.

### Fixed

- BREAKING: `KeyValuePairs::insert` (as well as `Labels::`/`Annotations::` via it) now overwrites
  the old value if the key already exists. Previously, `iter()` would return *both* values in
  lexicographical order (causing further conversions like `Into<BTreeMap>` to prefer the maximum
  value) ([#888]).

### Removed

- BREAKING: The `CustomResourceExt` trait doesn't provide a `generate_yaml_schema` function any
  more. Instead, use the high-level functions to write the schema to a file, write it to stdout or
  use it as a `String` ([#883]).

[#883]: https://github.com/stackabletech/operator-rs/pull/883
[#885]: https://github.com/stackabletech/operator-rs/pull/885
[#888]: https://github.com/stackabletech/operator-rs/pull/888
[#893]: https://github.com/stackabletech/operator-rs/pull/893

## [0.78.0] - 2024-09-30

### Added

- Add Kerberos AuthenticationProvider ([#880]).

[#880]: https://github.com/stackabletech/operator-rs/pull/880

## [0.77.1] - 2024-09-27

### Fixed

- Fix always returning an error stating that volumeMounts are colliding. Instead move the error
  creation to the correct location within an `if` statement ([#879]).

[#879]: https://github.com/stackabletech/operator-rs/pull/879

## [0.77.0] - 2024-09-26

### Fixed

- Fix the logback configuration for logback versions from 1.3.6/1.4.6 to 1.3.11/1.4.11 ([#874]).
- BREAKING: Avoid colliding volumes and mounts by only adding volumes or mounts if they do not already exist. This makes functions such as `PodBuilder::add_volume` or `ContainerBuilder::add_volume_mount` as well as related ones fallible ([#871]).

### Changed

- BREAKING: Remove the `unique_identifier` argument from `ResolvedS3Connection::add_volumes_and_mounts`, `ResolvedS3Connection::volumes_and_mounts` and `ResolvedS3Connection::credentials_mount_paths` as it is not needed anymore ([#871]).

[#871]: https://github.com/stackabletech/operator-rs/pull/871
[#874]: https://github.com/stackabletech/operator-rs/pull/874

## [0.76.0] - 2024-09-19

### Added

- BREAKING: Add `HostName` type and use it within LDAP and OIDC AuthenticationClass as well as S3Connection ([#863]).

### Changed

- BREAKING: The TLS verification struct now resides in the `commons::tls_verification` module, instead of being placed below `commons::authentication::tls` ([#863]).
- BREAKING: Rename the `Hostname` type to `DomainName` to be consistent with RFC 1123 ([#863]).

### Fixed

- BREAKING: The fields `bucketName`, `connection` and `host` on `S3BucketSpec`, `InlinedS3BucketSpec` and `S3ConnectionSpec` are now mandatory. Previously operators errored out in case these fields where missing ([#863]).

[#863]: https://github.com/stackabletech/operator-rs/pull/863

## [0.75.0] - 2024-09-19

### Added

- Add `Hostname` and `KerberosRealmName` types extracted from secret-operator ([#851]).
- Add support for listener volume scopes to `SecretOperatorVolumeSourceBuilder` ([#858]).

### Changed

- BREAKING: `validation` module now uses typed errors ([#851]).
- Set `checkIncrement` to 5 seconds in Logback config ([#853]).
- Bump Rust dependencies and enable Kubernetes 1.31 (via `kube` 0.95.0) ([#867]).

### Fixed

- Fix the CRD description of `ClientAuthenticationDetails` to not contain internal Rust doc, but a public CRD description ([#846]).
- `StackableAffinity` fields are no longer erroneously marked as required ([#855]).
- BREAKING: `ClusterResources` will now only consider deleting objects that are marked as directly owned (via `.metadata.ownerReferences`) ([#862]).

[#846]: https://github.com/stackabletech/operator-rs/pull/846
[#851]: https://github.com/stackabletech/operator-rs/pull/851
[#853]: https://github.com/stackabletech/operator-rs/pull/853
[#855]: https://github.com/stackabletech/operator-rs/pull/855
[#858]: https://github.com/stackabletech/operator-rs/pull/858
[#862]: https://github.com/stackabletech/operator-rs/pull/862
[#867]: https://github.com/stackabletech/operator-rs/pull/867

## [0.74.0] - 2024-08-22

### Added

- Add `iter::reverse_if` helper ([#838]).
- Add two new constants `CONFIG_OVERRIDE_FILE_HEADER_KEY` and `CONFIG_OVERRIDE_FILE_FOOTER_KEY` ([#843]).

### Changed

- BREAKING: Replace `lazy_static` with `std::cell::LazyCell` (the original implementation was done in [#827] and reverted in [#835]) ([#840]).
- BREAKING: Swap priority order of role group config and role overrides in configuration merging to prioritize overrides in general ([#841]).

[#838]: https://github.com/stackabletech/operator-rs/pull/838
[#840]: https://github.com/stackabletech/operator-rs/pull/840
[#841]: https://github.com/stackabletech/operator-rs/pull/841
[#843]: https://github.com/stackabletech/operator-rs/pull/843

## [0.73.0] - 2024-08-09

### Added

- Rollout tracker for `StatefulSet` ([#833]).

### Changed

- Reverted [#827], in order to restore Rust 1.79 compatibility for now ([#835]), re-opened in ([#840]).

### Fixed

- Invalid CRD schema for `StackableAffinity` contents. This was caused by the fields being optional and defaulting to `null`, while the custom schema marked the field as required ([#836]).

[#833]: https://github.com/stackabletech/operator-rs/pull/833
[#835]: https://github.com/stackabletech/operator-rs/pull/835
[#836]: https://github.com/stackabletech/operator-rs/pull/836

## [0.72.0] - 2024-08-05

### Changed

- BREAKING: Replace `lazy_static` with `std::cell::LazyCell` ([#827], [#835], [#840]).
- BREAKING: Convert `podOverrides` and `affinity` fields to take any arbitrary
  YAML input, rather than using the underlying schema. With this change, one of
  the larger CRDs, like the Druid CRD went down in size from `2.4MB` to `288K`
  (a 88% reduction). One downside is that user input is not checked to be a
  valid `PodTemplateSpec`, `PodAffinity`, `PodAntiAffinity` and `NodeAffinity`
  any more. However, checks can be re-added by using validation webhooks if
  needed. This change should not be breaking for the user and is a preparation
  for CRD versioning. ([#821]).

[#821]: https://github.com/stackabletech/operator-rs/pull/821
[#827]: https://github.com/stackabletech/operator-rs/pull/827

## [0.71.0] - 2024-07-29

### Added

- Added support for logging to files ([#814]).

### Changed

- Changed OPA Bundle Builder Vector config to read from the new log-to-file setup ([#814]).

[#814]: https://github.com/stackabletech/operator-rs/pull/814

## [0.70.0] - 2024-07-10

### Added

- Added `ProductImage::product_version` utility function ([#817], [#818])

### Changed

- BREAKING: Bump `kube` to 0.92.0. This required changes in a unit test, because
  the `kube::runtime::watcher::Event` enum introduced new and renamed some
  variants. Also see the following additional resources ([#804]).
  - [Blog Post - Breaking Change](https://kube.rs/blog/2024/06/11/watcher-memory-improvements/#breaking-change)
  - [kube#1494](https://github.com/kube-rs/kube/pull/1494)
  - [kube#1504](https://github.com/kube-rs/kube/pull/1504)
- Upgrade opentelemetry crates ([#811]).
- Bump rust-toolchain to 1.79.0 ([#822]).

### Fixed

- Product image selection pull request version override now only applies to pull requests ([#812]).
- OPA bundle builder logs without a log message are marked with the
  error "Message not found." instead of "Log event not parsable" ([#819]).

[#804]: https://github.com/stackabletech/operator-rs/pull/804
[#811]: https://github.com/stackabletech/operator-rs/pull/811
[#812]: https://github.com/stackabletech/operator-rs/pull/812
[#817]: https://github.com/stackabletech/operator-rs/pull/817
[#818]: https://github.com/stackabletech/operator-rs/pull/818
[#819]: https://github.com/stackabletech/operator-rs/pull/819
[#822]: https://github.com/stackabletech/operator-rs/pull/822

## [0.69.3] - 2024-06-12

### Fixed

- Processing of corrupted log events fixed; If errors occur, the error
  messages are added to the log event ([#802]).

[#802]: https://github.com/stackabletech/operator-rs/pull/802

## [0.69.2] - 2024-06-10

### Changed

- Change `strum::Display` output format for `LogLevel` to uppercase ([#808]).

[#808]: https://github.com/stackabletech/operator-rs/pull/808

## [0.69.1] - 2024-06-10

### Added

- Derive `strum::Display` for `LogLevel`([#805]).

[#805]: https://github.com/stackabletech/operator-rs/pull/805

## [0.69.0] - 2024-06-03

### Added

- Add functionality to convert LogLevel to an OPA log level ([#798]).
- BREAKING: Add labels to listener volume builder. `PodBuilder::add_listener_volume_by_listener_class`, `PodBuilder::add_listener_volume_by_listener_name` and `ListenerOperatorVolumeSourceBuilder::new` now require you to pass the labels for the created volumes ([#799]).

[#798]: https://github.com/stackabletech/operator-rs/pull/798
[#799]: https://github.com/stackabletech/operator-rs/pull/799

## [0.68.0] - 2024-05-22

- Support specifying externalTrafficPolicy in Services created by listener-operator ([#773], [#789], [#791]).

[#773]: https://github.com/stackabletech/operator-rs/pull/773
[#789]: https://github.com/stackabletech/operator-rs/pull/789
[#791]: https://github.com/stackabletech/operator-rs/pull/791

## [0.67.1] - 2024-05-08

### Added

- Add `InvalidProductSpecificConfiguration` variant in
  `stackable_operator::product_config_util::Error` enum ([#782]).

### Changed

- Bump Rust dependencies and GitHub Actions ([#782]).
- Bump GitHub workflow actions ([#772]).
- Revert `zeroize` version bump ([#772]).

[#772]: https://github.com/stackabletech/operator-rs/pull/772
[#782]: https://github.com/stackabletech/operator-rs/pull/782

## [0.67.0] - 2024-04-25

### Changed

- Bump kube to 0.89.0 and update all dependencies ([#762]).
- BREAKING: Bump k8s compilation version to `1.29`. Also bump all dependencies.
  There are some breaking changes in k8s-openapi, e.g. PVCs now have `VolumeResourceRequirements` instead of `ResourceRequirements`,
  and `PodAffinityTerm` has two new fields `match_label_keys` and `mismatch_label_keys` ([#769]).

### Removed

- BREAKING: Remove `thiserror` dependency, and deprecated builder exports ([#761])

[#761]: https://github.com/stackabletech/operator-rs/pull/761
[#762]: https://github.com/stackabletech/operator-rs/pull/762
[#769]: https://github.com/stackabletech/operator-rs/pull/769

## [0.66.0] - 2024-03-26

### Changed

- Implement `PartialEq` for most *Snafu* Error enums ([#757]).
- Update Rust to 1.77 ([#759])

### Fixed

- Fix wrong schema (and thus CRD) for `config.affinity.nodeSelector` ([#752]).

[#752]: https://github.com/stackabletech/operator-rs/pull/752
[#757]: https://github.com/stackabletech/operator-rs/pull/757
[#759]: https://github.com/stackabletech/operator-rs/pull/759

## [0.65.0] - 2024-03-25

### Added

- Add `stackable_webhook` crate which provides utilities to create webhooks with TLS termination ([#730]).
- Add `ConversionReview` re-export in `stackable_webhook` crate ([#749]).

[#730]: https://github.com/stackabletech/operator-rs/pull/730
[#749]: https://github.com/stackabletech/operator-rs/pull/749

### Changed

- Remove `resources` key from `DynamicValues` struct ([#734]).
- Bump `opentelemetry`, `opentelemetry_sdk`, `opentelemetry-jaeger`, and `tracing-opentelemetry` Rust dependencies
  ([#753]).
- Bump GitHub workflow actions ([#754]).

[#734]: https://github.com/stackabletech/operator-rs/pull/734
[#753]: https://github.com/stackabletech/operator-rs/pull/753
[#754]: https://github.com/stackabletech/operator-rs/pull/754

### Fixed

- Fixed incorrect time calculation ([#735]).

[#735]: https://github.com/stackabletech/operator-rs/pull/735

## [0.64.0] - 2024-01-31

### Added

- Derive `Hash` and `Ord` instances for `AuthenticationClassProvider`,
  so that duplicates can be detected ([#731]).

[#731]: https://github.com/stackabletech/operator-rs/pull/731

## [0.63.0] - 2024-01-26

### Added

- Add Serde `Deserialize` and `Serialize` support for `CpuQuantity` and `MemoryQuantity` ([#724]).
- Add `DynamicValues` struct to work with operator `values.yaml` files during runtime ([#723]).

[#723]: https://github.com/stackabletech/operator-rs/pull/723
[#724]: https://github.com/stackabletech/operator-rs/pull/724

### Changed

- Change Deref target of `KeyPrefix` and `KeyName` from `String` to `str` ([#725]).
- Add Stackable vendor label `stackable.tech/vendor: Stackable` to recommended labels ([#728]).

[#725]: https://github.com/stackabletech/operator-rs/pull/725
[#728]: https://github.com/stackabletech/operator-rs/pull/728

## [0.62.0] - 2024-01-19

### Added

- Added `Option::as_ref_or_else` to `utils` ([#717]).
- Add `iter()` methods to `KeyValuePairs<T>`, and delegate iter() for `Labels`, and `Annotations` ([#720]).
- Implement `IntoIterator` for `KeyValuePairs<T>`, `Labels` and `Annotations` ([#720]).
- Added `ListenerOperatorVolumeSourceBuilder::build_pvc` ([#719]).
- Added `Logging::for_container` ([#721]).

### Changed

- Split `utils` into submodules ([#717]).
- Bump rust to 1.75.0 ([#720]).
- Renamed `ListenerOperatorVolumeSourceBuilder::build` to `::build_ephemeral` ([#719]).

[#717]: https://github.com/stackabletech/operator-rs/pull/717
[#720]: https://github.com/stackabletech/operator-rs/pull/720
[#719]: https://github.com/stackabletech/operator-rs/pull/719
[#721]: https://github.com/stackabletech/operator-rs/pull/721

## [0.61.0] - 2024-01-15

### Added

- Add `TryFrom<[(K, V); N]>` implementation for `Annotations` and `Labels` ([#711]).
- Add `parse_insert` associated function for `Annotations` and `Labels` ([#711]).
- Add generic types for `TryFrom<BTreeMap<K, V>>` impl ([#714]).
- Add `TryFromIterator` trait, which tries to construct `Self` from an iterator. It is a falliable version of
  `FromIterator` ([#715]).
- Add `TryFromIterator` impl for `Labels` and `Annotations` ([#715]).

### Changed

- Adjust `try_insert` for `Annotations` and `Labels` slightly ([#711]).

[#711]: https://github.com/stackabletech/operator-rs/pull/711
[#714]: https://github.com/stackabletech/operator-rs/pull/714
[#715]: https://github.com/stackabletech/operator-rs/pull/715

## [0.60.1] - 2024-01-04

### Fixed

- Let `ldap::AuthenticationProvider::add_volumes_and_mounts` also add the needed TLS volumes. This functionality was removed in [#680] and causes kuttl tests to fail, as the ca-cert volume and mount where missing. This patch restores the previous behavior (of adding needed TLS volumes) ([#708]).

[#708]: https://github.com/stackabletech/operator-rs/pull/708

## [0.60.0] - 2024-01-03

### Added

- Add LDAP AuthenticationClassProvider `endpoint_url()` method so each operator doesn't have to construct it. ([#705])

[#705]: https://github.com/stackabletech/operator-rs/pull/705

## [0.59.0] - 2023-12-21 🌲

### Added

- Add `stackble_operator::kvp` module and types to allow validated construction of key/value pairs, like labels and
  annotations. Most users want to use the exported type aliases `Label` and `Annotation` ([#684]).

### Changed

- Move `stackable_operator::label_selector::convert_label_selector_to_query_string` into `kvp` module. The conversion
  functionality now is encapsulated in a new trait `LabelSelectorExt`. An instance of a `LabelSelector` can now be
  converted into a query string by calling the associated function `ls.to_query_string()` ([#684]).
- BREAKING: Remove legacy node selector on `RoleGroup` ([#652]).

[#684]: https://github.com/stackabletech/operator-rs/pull/684
[#652]: https://github.com/stackabletech/operator-rs/pull/652

## [0.58.1] - 2023-12-12

### Added

- More CRD documentation ([#697]).

[#697]: https://github.com/stackabletech/operator-rs/pull/697

## [0.58.0] - 2023-12-04

### Added

- Add `oidc::AuthenticationProvider`. This enables users to deploy a new `AuthenticationClass` for OIDC providers like
  Keycloak, Okta or Auth0 ([#680]).
- Add a common `ClientAuthenticationDetails` struct, which provides common fields and functions to specify
  authentication options on product cluster level. Additionally, the PR also adds `ClientAuthenticationConfig`,
  `oidc::ClientAuthenticationOptions`, and `ldap::ClientAuthenticationOptions` ([#680]).

### Changed

- BREAKING: Change the naming of all authentication provider structs. It is now required to import them using the
  module. So imports change from `...::authentication::LdapAuthenticationProvider` to
  `...::authentication::ldap::AuthenticationProvider` for example ([#680]).
- BREAKING: Move TLS related structs into the `tls` module. Imports need to be adjusted accordingly ([#680]).

### Fixed

- Fixed appVersion label in case container images contain a hash, such as `docker.stackable.tech/stackable/nifi@sha256:85fa483aa99b9997ce476b86893ad5ed81fb7fd2db602977eb8c42f76efc109`. Also added a test-case to ensure we support images containing hashes. This should be a rather cosmetic fix, images with hashes should have worked before anyway ([#690]).

[#680]: https://github.com/stackabletech/operator-rs/pull/680
[#690]: https://github.com/stackabletech/operator-rs/pull/690

## [0.57.0] - 2023-12-04

### Changed

- BREAKING: The `CustomResourceExt` functions now take the Operator version as an argument.
  It replaces `DOCS_BASE_URL_PLACEHOLDER` in doc strings with a link to URL base, so
  `DOCS_BASE_URL_PLACEHOLDER/druid/` turns into `https://docs.stackable.tech/home/nightly/druid/`
  in the nightly operator ([#689]).

[#689]: https://github.com/stackabletech/operator-rs/pull/689

## [0.56.2] - 2023-11-23

### Added

- More documentation for CRD structs ([#687]).

[#687]: https://github.com/stackabletech/operator-rs/pull/687

## [0.56.1] - 2023-11-23

### Changed

- Update `kube` to `0.87.1` as version `0.86.0` was yanked ([#685]).

[#685]: https://github.com/stackabletech/operator-rs/pull/685

## [0.56.0] - 2023-10-31 👻

### Added

- Added `COMMON_BASH_TRAP_FUNCTIONS`, which can be used to write a Vector shutdown trigger file after the main
  application stopped ([#681]).

### Changed

- BREAKING: Rename `product_logging::framework::shutdown_vector_command` to `create_vector_shutdown_file_command` and
  added `remove_vector_shutdown_file_command` ([#681]).
- BREAKING: Remove re-export of `product_config`, update `product_config` to `0.6.0` ([#682]).

### Fixed

- Fix Docker image tag parsing when user specifies custom image ([#677]).

[#677]: https://github.com/stackabletech/operator-rs/pull/677
[#681]: https://github.com/stackabletech/operator-rs/pull/681
[#682]: https://github.com/stackabletech/operator-rs/pull/682

## [0.55.0] - 2023-10-16

### Added

- Mark the following functions as `const` ([#674]):
  - `ClusterResourceApplyStrategy::delete_orphans`
  - `LdapAuthenticationProvider::default_port`
  - `LdapAuthenticationProvider::use_tls`
  - `ListenerSpec::default_publish_not_ready_addresses`
  - `OpaApiVersion::get_data_api`
  - `CpuQuantity::from_millis`
  - `CpuQuantity::as_milli_cpus`
  - `BinaryMultiple::exponential_scale_factor`
  - `BinaryMultiple::get_smallest`
  - `MemoryQuantity::from_gibi`
  - `MemoryQuantity::from_mebi`
  - `ClusterCondition::is_good`
  - `ClusterOperationsConditionBuilder::new`
  - `commons::pdb::default_pdb_enabled`
- Add interoperability between the `time` crate and the `stackable_operator::time::Duration` struct. This is opt-in and
  requires the `time` feature to be enabled. Additionally, adds `Add`, `AddAssign`, `Sub`, and `SubAssign` operations
  between `Duration` and `std::time::Instant`. Further adds a new helper function `Duration::now_utc` which calculates
  the duration from the unix epoch (1970-01-01 00:00:00) until now ([#671]).

### Changed

- BREAKING: Rename top-level `duration` module to `time`. Imports now use `stackable_operator::time::Duration` for
  example ([#671]).
- Convert the format of the Vector configuration from TOML to YAML ([#670]).
- BREAKING: Rename `PodBuilder::termination_grace_period_seconds` to `termination_grace_period`, and change it to take `Duration` struct ([#672]).

### Fixed

- stackable-operator-derive: Add descriptions to derived Fragment structs ([#675]).

[#670]: https://github.com/stackabletech/operator-rs/pull/670
[#671]: https://github.com/stackabletech/operator-rs/pull/671
[#672]: https://github.com/stackabletech/operator-rs/pull/672
[#674]: https://github.com/stackabletech/operator-rs/pull/674
[#675]: https://github.com/stackabletech/operator-rs/pull/675

## [0.54.0] - 2023-10-10

### Changed

- impl `Atomic` for `Duration` ([#668]).

[#668]: https://github.com/stackabletech/operator-rs/pull/668

## [0.53.0] - 2023-10-09

### Changed

- Add duration overflow check ([#665]).
- Add `Duration::from_millis`, `Duration::from_minutes_unchecked`, `Duration::from_hours_unchecked` and
  `Duration::from_days_unchecked` ([#657]).

[#657]: https://github.com/stackabletech/operator-rs/pull/657
[#665]: https://github.com/stackabletech/operator-rs/pull/665

## [0.52.1] - 2023-10-05

Only rust documentation was changed.

## [0.52.0] - 2023-10-05

### Changed

- BREAKING: Make roleConfig customizable by making the `Role` struct generic over the `roleConfig` ([#661]).

[#661]: https://github.com/stackabletech/operator-rs/pull/661

## [0.51.1] - 2023-09-26

### Fixed

- Fix a typo in the documentation of the `PdbConfig` struct ([#659]).

[#659]: https://github.com/stackabletech/operator-rs/pull/659

## [0.51.0] - 2023-09-25

### Added

- Add `PdbConfig` struct and `PodDisruptionBudgetBuilder` ([#653]).

[#653]: https://github.com/stackabletech/operator-rs/pull/653

## [0.50.0] - 2023-09-18

- Add `Duration` capable of parsing human-readable duration formats ([#647]).

[#647]: https://github.com/stackabletech/operator-rs/pull/647

## [0.49.0] - 2023-09-15

### Added

- `PodListeners` CRD ([#644]).
- Add support for tls pkcs12 password to secret operator volume builder ([#645]).

### Changed

- Derive `Eq` and `Copy` where applicable for listener CRDs ([#644]).
- Bump `kube` to `0.86.0` and Kubernetes version to `1.28` ([#648]).

[#644]: https://github.com/stackabletech/operator-rs/pull/644
[#645]: https://github.com/stackabletech/operator-rs/pull/645
[#648]: https://github.com/stackabletech/operator-rs/pull/648

## [0.48.0] - 2023-08-18

### Added

- Add `PodBuilder::termination_grace_period_seconds` ([#641]).
- Add support for adding `lifecycle`s to `ContainerBuilder` ([#641]).

[#641]: https://github.com/stackabletech/operator-rs/pull/641

## [0.47.0] - 2023-08-16

### Added

- Implement `Display` for `MemoryQuantity` ([#638]).
- Implement `Sum` for `CpuQuantity` and `MemoryQuantity` ([#634]).

### Changed

- Switch from `openssl` to `rustls` ([#635]).
- Bump `product-config`` 0.4.0 -> 0.5.0 ([#639]).

### Fixed

- Fixed buggy `Div`, `SubAssign` and `AddAssign` for `MemoryQuantity` when left and right side had different units ([#636], [#637]).

[#634]: https://github.com/stackabletech/operator-rs/pull/634
[#635]: https://github.com/stackabletech/operator-rs/pull/635
[#636]: https://github.com/stackabletech/operator-rs/pull/636
[#637]: https://github.com/stackabletech/operator-rs/pull/637
[#638]: https://github.com/stackabletech/operator-rs/pull/638
[#639]: https://github.com/stackabletech/operator-rs/pull/639

## [0.46.0] - 2023-08-08

### Changed

- Bump all dependencies (including kube and k8s-openapi) ([#632]).
- Bump Rust version to 0.71.0 ([#633]).
- Refactor Cargo.toml's to share workspace configuration, such as version and license ([#633]).

[#632]: https://github.com/stackabletech/operator-rs/pull/632
[#633]: https://github.com/stackabletech/operator-rs/pull/633

## [0.45.1] - 2023-08-01

### Fixed

- Support PR versions in automatic stackableVersion - ([#619]) falsely assumed the binaries in `-pr` versions
  have the version `0.0.0-dev` ([#629]).

[#629]: https://github.com/stackabletech/operator-rs/pull/629

## [0.45.0] - 2023-08-01

### Changed

- BREAKING: ProductImageSelection now defaults `stackableVersion` to
  operator version ([#619]).
- Default `pullPolicy` to operator `Always` ([#619]).
- BREAKING: Assume that the Vector executable is located in a directory
  which is specified in the PATH environment variable. This is the case
  if Vector is installed via RPM ([#625]).
- BREAKING: Update `product_logging::framework::create_vector_config` to
  be compatible with Vector version 0.31.0. The product image must
  contain Vector 0.31.x ([#625]).

### Fixed

- Fix the log level filter for the Vector container. If the level of the
  ROOT logger was set to TRACE and the level of the file logger was set
  to DEBUG then TRACE logs were written anyway ([#625]).

[#619]: https://github.com/stackabletech/operator-rs/pull/619
[#625]: https://github.com/stackabletech/operator-rs/pull/625

## [0.44.0] - 2023-07-13

### Added

- Add a function for calculating the size limit of log volumes ([#621]).

[#621]: https://github.com/stackabletech/operator-rs/pull/621

## [0.43.0] - 2023-07-06

### Added

- Secrets can now be requested in a custom format ([#610]).

### Changed

- Make pod overrides usable independently of roles (like in the case of the Spark operator) ([#616])

[#610]: https://github.com/stackabletech/operator-rs/pull/610
[#616]: https://github.com/stackabletech/operator-rs/pull/616

## [0.42.2] - 2023-06-27

### Fixed

- Strip out documentation from pod override templates ([#611]).

[#611]: https://github.com/stackabletech/operator-rs/pull/611

## [0.42.1] - 2023-06-15

### Fixed

- Let `PodBuilder::build_template` return `PodTemplateSpec` instead of `OperatorResult<PodTemplateSpec>` (fixup of #598) ([#605]).

[#605]: https://github.com/stackabletech/operator-rs/pull/605

## [0.42.0] - 2023-06-15

### Added

- Add a new `ResourceRequirementsBuilder` to more easily build resource requirements in a controlled and well defined
  way. ([#598]).
- Add podOverrides to common struct CommonConfiguration ([#601]).
- All the operators now must respect the new `podOverrides` attribute! ([#601]).
- Support ClusterIP type in services created by listener-operator ([#602]).

### Changed

- Set default resource limits on `PodBuilder::add_init_container` ([#598]).
- Made `StaticAuthenticationProvider` fields public ([#597]).
- [INTERNALLY BREAKING]: Moved `StaticAuthenticationProvider`, `LdapAuthenticationProvider`, `TlsAuthenticationProvider`
  to its own module `authentication` ([#597]).

[#597]: https://github.com/stackabletech/operator-rs/pull/597
[#598]: https://github.com/stackabletech/operator-rs/pull/598
[#601]: https://github.com/stackabletech/operator-rs/pull/601
[#602]: https://github.com/stackabletech/operator-rs/pull/602

## [0.41.0] - 2023-04-20

### Changed

- kube: 0.78.0 -> 0.82.2 ([#589]).
- k8s-openapi: 0.17.0 -> 0.18.0 ([#589]).

[#589]: https://github.com/stackabletech/operator-rs/pull/589

## [0.40.2] - 2023-04-12

### Fixed

- Added clean up for `Job` to cluster resources `delete_orphaned_resources` ([#583]).

[#583]: https://github.com/stackabletech/operator-rs/pull/583

## [0.40.1] - 2023-04-12

### Added

- `ClusterResources` implementation for `Job` ([#581]).
- Helper methods to generate RBAC `ServiceAccount` and `ClusterRole` names ([#581]).

[#581]: https://github.com/stackabletech/operator-rs/pull/581

## [0.40.0] - 2023-04-11

### Added

- BREAKING: Added ownerreferences and labels to `build_rbac_resources` ([#579]).

[#579]: https://github.com/stackabletech/operator-rs/pull/579

## [0.39.1] - 2023-04-07

### Fixed

- Fix the parsing of log4j and logback files in the Vector configuration, avoid
  rounding errors in the timestamps, and improve the handling of unparseable
  log events ([#577]).

[#577]: https://github.com/stackabletech/operator-rs/pull/577

## [0.39.0] - 2023-03-31

### Added

- status::condition module to compute the cluster resource status ([#571]).
- Helper function to build RBAC resources ([#572]).
- Add `ClusterResourceApplyStrategy` to `ClusterResource` ([#573]).
- Add `ClusterOperation` common struct with `reconcilation_paused` and `stopped` flags ([#573]).

[#571]: https://github.com/stackabletech/operator-rs/pull/571
[#572]: https://github.com/stackabletech/operator-rs/pull/572
[#573]: https://github.com/stackabletech/operator-rs/pull/573

## [0.38.0] - 2023-03-20

### Added

- Helper function to add a restart_policy to PodBuilder ([#565]).
- Add helper function `SecretOperatorVolumeSourceBuilder::with_kerberos_service_name` ([#568]).

[#565]: https://github.com/stackabletech/operator-rs/pull/565
[#568]: https://github.com/stackabletech/operator-rs/pull/568

## [0.37.0] - 2023-03-06

### Added

- Vector sources and transforms for OPA bundle builder and OPA json logs ([#557]).

[#557]: https://github.com/stackabletech/operator-rs/pull/557

## [0.36.1] - 2023-02-27

### Fixed

- Fix legacy selector overwriting nodeAffinity and nodeSelector ([#560]).

[#560]: https://github.com/stackabletech/operator-rs/pull/560

## [0.36.0] - 2023-02-17

### Added

- Added commons structs as well as helper functions for Affinity ([#556]).

[#556]: https://github.com/stackabletech/operator-rs/pull/556

## [0.35.0] - 2023-02-13

### Added

- Added airlift json source and airlift json transform to vector.toml ([#553]).

[#553]: https://github.com/stackabletech/operator-rs/pull/553

## [0.34.0] - 2023-02-06

### Added

- Processing of Python log files added to the Vector agent configuration ([#539]).
- Command added to shutdown Vector, e.g. after a job is finished ([#539]).

### Changed

- clap: 4.0.32 -> 4.1.4 ([#549]).
- tokio: 1.24.1 -> 1.25.0 ([#550]).

[#539]: https://github.com/stackabletech/operator-rs/pull/539
[#549]: https://github.com/stackabletech/operator-rs/pull/549
[#550]: https://github.com/stackabletech/operator-rs/pull/550

## [0.33.0] - 2023-02-01

### Added

- New `CpuQuantity` struct to represent CPU quantities ([#544]).
- Implemented `Add`, `Sub`, `Div`, `PartialOrd` and more for `MemoryQuantity` ([#544]).

### Changed

- Deprecated `to_java_heap` and `to_java_heap_value` ([#544]).
- BREAKING: For all products using logback. Added additional optional parameter to `create_logback_config()` to supply custom configurations not covered via the standard log configuration ([#546]).

[#544]: https://github.com/stackabletech/operator-rs/pull/544
[#546]: https://github.com/stackabletech/operator-rs/pull/546

## [0.32.1] - 2023-01-24

### Fixed

- Parsing of timestamps in log4j2 log events made fail-safe ([#542]).

## [0.32.0] - 2023-01-24

### Added

- Added method to create log4j2 config properties to product logging ([#540]).

[#540]: https://github.com/stackabletech/operator-rs/pull/540

## [0.31.0] - 2023-01-16

### Added

- Extended the `LdapAuthenticationProvider` with functionality to build add Volumes and Mounts to PodBuilder and ContainerBuilder ([#535]).
- Extended the `PodBuilder` with `add_volume_with_empty_dir` utility function ([#536]).

[#535]: https://github.com/stackabletech/operator-rs/pull/535
[#536]: https://github.com/stackabletech/operator-rs/pull/536

## [0.30.2] - 2022-12-20

### Changed

- Disable Vector agent by default ([#526]).
- Bump kube to 0.78.0 and k8s-openapi to 0.17.0. Bump k8s version from 1.24 to 1.26 ([#533]).

[#526]: https://github.com/stackabletech/operator-rs/pull/526
[#533]: https://github.com/stackabletech/operator-rs/pull/533

## [0.30.1] - 2022-12-19

### Removed

- Removed `affinity` property from the RoleGroup that was added in [#520] but not intended to be there ([#552]).

[#552]: https://github.com/stackabletech/operator-rs/pull/522

## [0.30.0] - 2022-12-19

### Added

- Extended the `PodBuilder` with `pod_affinity`, `pod_anti_affinity`, `node_selector` and their `*_opt` variants ([#520]).

[#520]: https://github.com/stackabletech/operator-rs/pull/520

## [0.29.0] - 2022-12-16

### Added

- Modules for log aggregation added ([#517]).

[#517]: https://github.com/stackabletech/operator-rs/pull/517

## [0.28.0] - 2022-12-08

### Added

- Added `AuthenticationClass` provider static ([#514]).

[#514]: https://github.com/stackabletech/operator-rs/pull/514

## [0.27.1] - 2022-11-17

### Changed

- Changed the separator character between operator and controller names ([#507]).

[#507]: https://github.com/stackabletech/operator-rs/pull/507

## [0.27.0] - 2022-11-14

### Added

- Added product image selection struct ([#476]).

### Changed

- BREAKING: `get_recommended_labels` and `with_recommended_labels` now takes a struct of named arguments ([#501]).
- BREAKING: `get_recommended_labels` (and co) now takes the operator and controller names separately ([#492]).
- BREAKING: `ClusterResources` now takes the operator and controller names separately ([#492]).
  - When upgrading, please use FQDN-style names for the operators (`{operator}.stackable.tech`).
- Bump kube to `0.76.0` ([#476]).
- Bump opentelemetry crates ([#502]).
- Bump clap to 4.0 ([#503]).

[#476]: https://github.com/stackabletech/operator-rs/pull/476
[#492]: https://github.com/stackabletech/operator-rs/pull/492
[#501]: https://github.com/stackabletech/operator-rs/pull/501
[#502]: https://github.com/stackabletech/operator-rs/pull/502
[#503]: https://github.com/stackabletech/operator-rs/pull/503

## [0.26.1] - 2022-11-08

### Added

- Builder for `EphemeralVolumeSource`s added which are used by the listener-operator ([#496]).
- Exposed parser for Kubernetes `Quantity` values ([#499]).

[#496]: https://github.com/stackabletech/operator-rs/pull/496
[#499]: https://github.com/stackabletech/operator-rs/pull/499

## [0.26.0] - 2022-10-20

### Added

- Added new Fragment (partial configuration) machinery ([#445]).

### Changed

- kube-rs: 0.74.0 -> 0.75.0 ([#490]).
- BREAKING: `Client` methods now take the namespace as a `&str` (for namespaced resources) or
  `&()` (for cluster-scoped resources), rather than always taking an `Option<&str>` ([#490]).

[#445]: https://github.com/stackabletech/operator-rs/pull/445
[#490]: https://github.com/stackabletech/operator-rs/pull/490

## [0.25.3] - 2022-10-13

### Added

- Extended `ClusterResource` with `Secret`, `ServiceAccount` and `RoleBinding` ([#485]).

[#485]: https://github.com/stackabletech/operator-rs/pull/485

## [0.25.2] - 2022-09-27

This is a rerelease of 0.25.1 which some last-minute incompatible API changes to the additions that would have been released in 0.25.1.

### Changed

- Use Volume as the primary mechanism for directing Listener traffic, rather than labels ([#474]).

[#474]: https://github.com/stackabletech/operator-rs/pull/474

## ~~[0.25.1] - 2022-09-23~~ YANKED

### Added

- listener-operator CRDs ([#469]).

[#469]: https://github.com/stackabletech/operator-rs/pull/469

## [0.25.0] - 2022-08-23

### Added

- YAML module added with a function to serialize a data structure as an
  explicit YAML document. The YAML documents generated by the functions in
  `crd::CustomResourceExt` are now explicit documents and can be safely
  concatenated to produce a YAML stream ([#450]).

### Changed

- Objects are now streamed rather than polled when waiting for them to be deleted ([#452]).
- serde\_yaml 0.8.26 -> 0.9.9 ([#450])

[#450]: https://github.com/stackabletech/operator-rs/pull/450
[#452]: https://github.com/stackabletech/operator-rs/pull/452

## [0.24.0] - 2022-08-04

### Added

- Cluster resources can be added to a struct which determines the orphaned
  resources and deletes them ([#436]).
- Added `Client::get_opt` for trying to get an object that may not exist ([#451]).

### Changed

- BREAKING: The `managed_by` label must be passed explicitly to the
  `ObjectMetaBuilder::with_recommended_labels` function ([#436]).
- BREAKING: Renamed `#[merge(bounds)]` to `#[merge(bound)]` ([#445]).
- BREAKING: Added `Fragment` variants of most types in `stackable_operator::commons::resources` ([#445]).
  - serde impls have been moved to `FooFragment` variants, consumers that are not ready to use the full fragment machinery should switch to using these fragment variants.

[#436]: https://github.com/stackabletech/operator-rs/pull/436
[#451]: https://github.com/stackabletech/operator-rs/pull/451

## [0.23.0] - 2022-07-26

### Added

- Add `AuthenticationClass::resolve` helper function ([#432]).

### Changed

- BREAKING:kube `0.73.1` -> `0.74.0` ([#440]). Deprecate `ResourceExt::name` in favour of safe `name_*` alternatives. [kube-#945]
- `ContainerBuilder::new` validates container name to be RFC 1123-compliant ([#447]).

[#432]: https://github.com/stackabletech/operator-rs/pull/432
[#440]: https://github.com/stackabletech/operator-rs/pull/440
[#447]: https://github.com/stackabletech/operator-rs/pull/447
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

- BREAKING: kube 0.68 -> 0.69.1 ([#319], [#322]).

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
