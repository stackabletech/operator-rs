# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Fixed

- Don't error in case the `initial_reconcile` Receiver is dropped ([#XXXX]).

## [0.8.0] - 2025-12-32

### Added

- Add support for mutating webhooks ([#1119]).

### Changed

- BREAKING: Refactor the entire `WebhookServer` mechanism, so multiple webhooks can run in parallel.
  Put individual webhooks (currently `ConversionWebhook` and `MutatingWebhook`) behind the `Webhook` trait ([#1119]).

[#1119]: https://github.com/stackabletech/operator-rs/pull/1119

## [0.7.1] - 2025-10-31

### Fixed

- Force apply the CRD in the CRD maintainer ([#1116]).

  This ensures the operator will be the sole manager of the CRD (and all its fields) going forward.
  This avoids running into conflicts when CRDs were previously deployed by helm or stackablectl.

[#1116]: https://github.com/stackabletech/operator-rs/pull/1116

## [0.7.0] - 2025-10-16

### Added

- Add `CustomResourceDefinitionMaintainer` which applies and patches CRDs triggered by TLS
  certificate rotations of the `ConversionWebhookServer`. It additionally provides a `oneshot`
  channel which can for example be used to trigger creation/patching of any custom resources
  deployed by the operator ([#1099]).
- Add `ConversionWebhookServer::with_maintainer` which creates a conversion webhook server and a CRD
  maintainer ([#1099]).

### Changed

- BREAKING: `ConversionWebhookServer::new` now returns a pair of values ([#1099]):
  - The conversion webhook server itself
  - A `mpsc::Receiver<Certificate>` to provide consumers the newly generated TLS certificate
- BREAKING: Constants for ports, IP addresses and socket addresses are now associated constants on
  `(Conversion)WebhookServer` instead of free-standing ones ([#1099]).

### Removed

- BREAKING: The `maintain_crds` and `field_manager` fields in `ConversionWebhookOptions`
  are removed ([#1099]).

[#1099]: https://github.com/stackabletech/operator-rs/pull/1099

## [0.6.0] - 2025-09-09

### Added

- BREAKING: Support disabling CRD maintenance using a new boolean flag in `ConversionWebhookOptions` ([#1085]).

[#1085]: https://github.com/stackabletech/operator-rs/pull/1085

## [0.5.0] - 2025-08-21

### Changed

- BREAKING: Re-write the `ConversionWebhookServer`.
  It can now do CRD conversions, handle multiple CRDs and takes care of reconciling the CRDs ([#1066]).
- BREAKING: The `TlsServer` can now handle certificate rotation.
  To achieve this, a new `CertificateResolver` was added.
  Also, `TlsServer::new` now returns an additional `mpsc::Receiver<Certificate>`, so that the caller
  can get notified about certificate rotations happening ([#1066]).
- `stackable_webhook::Options` has been renamed to `stackable_webhook::WebhookOptions`, as well as
  `OptionsBuilder` to `WebhookOptionsBuilder` ([#1066]).

### Removed

- Remove `StatefulWebhookHandler` to reduce maintenance effort.
  Also, webhooks are ideally stateless, so that they can be scaled horizontally.
  It can be re-added once needed ([#1066]).

[#1066]: https://github.com/stackabletech/operator-rs/pull/1066

## [0.4.0] - 2025-07-10

### Fixed

- Don't pull in the `aws-lc-rs` crate, as this currently fails to build in `make run-dev` ([#1043]).

### Changed

- BREAKING: The constant `DEFAULT_IP_ADDRESS` has been renamed to `DEFAULT_LISTEN_ADDRESS` and binds to all
  addresses (instead of only loopback) by default. This was changed because all the webhooks
  deployed to Kubernetes (e.g. conversion or mutating - which this crate targets) need to be
  accessible by it, which is not the case when only using loopback.
  Also, the constant `DEFAULT_SOCKET_ADDR` has been renamed to `DEFAULT_SOCKET_ADDRESS` ([#1045]).

[#1043]: https://github.com/stackabletech/operator-rs/pull/1043
[#1045]: https://github.com/stackabletech/operator-rs/pull/1045

## [0.3.1] - 2024-07-10

### Changed

- Remove instrumentation of long running functions, add more granular instrumentation of futures. Adjust span and event levels ([#811]).
- Bump rust-toolchain to 1.79.0 ([#822]).

### Fixed

- Fix the extraction of `ConnectInfo` (data about the connection client) and
  the `Host` info (data about the server) in the `AxumTraceLayer`. This was
  previously not extracted correctly and thus not included in the OpenTelemetry
  compatible traces ([#806]).
- Spawn blocking code on a blocking thread ([#815]).

[#806]: https://github.com/stackabletech/operator-rs/pull/806
[#811]: https://github.com/stackabletech/operator-rs/pull/811
[#815]: https://github.com/stackabletech/operator-rs/pull/815
[#822]: https://github.com/stackabletech/operator-rs/pull/822

## [0.3.0] - 2024-05-08

### Added

- Instrument `WebhookServer` with `AxumTraceLayer`, add static healthcheck without instrumentation ([#758]).
- Add shutdown signal hander for the `WebhookServer` ([#767]).

### Changed

- Bump Rust dependencies and GitHub Actions ([#782]).
- Bump kube to 0.89.0 and update all dependencies ([#762]).
- BREAKING: Bump k8s compilation version to `1.29`. Also bump all dependencies.
  There are some breaking changes in k8s-openapi, e.g. PVCs now have `VolumeResourceRequirements` instead of `ResourceRequirements`,
  and `PodAffinityTerm` has two new fields `match_label_keys` and `mismatch_label_keys` ([#769]).
- Bump GitHub workflow actions ([#772]).
- Revert `zeroize` version bump ([#772]).

### Fixed

- Explicitly set the TLS provider for the ServerConfig, and enable "safe" protocols ([#778]).

[#758]: https://github.com/stackabletech/operator-rs/pull/758
[#762]: https://github.com/stackabletech/operator-rs/pull/762
[#767]: https://github.com/stackabletech/operator-rs/pull/767
[#769]: https://github.com/stackabletech/operator-rs/pull/769
[#772]: https://github.com/stackabletech/operator-rs/pull/772
[#778]: https://github.com/stackabletech/operator-rs/pull/778
[#782]: https://github.com/stackabletech/operator-rs/pull/782

## [0.2.0] - 2024-03-26

### Changed

- Implement `PartialEq` for most _Snafu_ Error enums ([#757]).
- Update Rust to 1.77 ([#759]).

[#757]: https://github.com/stackabletech/operator-rs/pull/757
[#759]: https://github.com/stackabletech/operator-rs/pull/759
