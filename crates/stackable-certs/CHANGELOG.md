# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- BREAKING: `CertificateAuthority::generate_leaf_certificate` (and `generate_rsa_leaf_certificate` and `generate_ecdsa_leaf_certificate`)
  now take an additional parameter `subject_alterative_dns_names`. The passed SANs are added to the generated certificate,
  this is needed when the HTTPS server is accessible on multiple DNS names and/or IPs.
  Pass an empty list (`[]`) to keep the existing behavior ([#1057]).
- BREAKING: The constant `DEFAULT_CA_VALIDITY_SECONDS` has been renamed to `DEFAULT_CA_VALIDITY` and now is of type `stackable_operator::time::Duration`.
  Also, the constant `ROOT_CA_SUBJECT` has been renamed to `SDP_ROOT_CA_SUBJECT` ([#1057]).
- Added the function `CertificateAuthority::ca_cert` to easily get the CA `Certificate` ([#1057]).

## [0.3.1] - 2024-07-10

### Changed

- Bump rust-toolchain to 1.79.0 ([#822]).

[#822]: https://github.com/stackabletech/operator-rs/pull/822
[#1057]: https://github.com/stackabletech/operator-rs/pull/1057

## [0.3.0] - 2024-05-08

### Changed

- Bump Rust dependencies and GitHub Actions ([#782]).
- Bump kube to 0.89.0 and update all dependencies ([#762]).
- BREAKING: Bump k8s compilation version to `1.29`. Also bump all dependencies.
  There are some breaking changes in k8s-openapi, e.g. PVCs now have `VolumeResourceRequirements` instead of `ResourceRequirements`,
  and `PodAffinityTerm` has two new fields `match_label_keys` and `mismatch_label_keys` ([#769]).
- Bump GitHub workflow actions ([#772]).
- Revert `zeroize` version bump ([#772]).

[#762]: https://github.com/stackabletech/operator-rs/pull/762
[#769]: https://github.com/stackabletech/operator-rs/pull/769
[#772]: https://github.com/stackabletech/operator-rs/pull/772
[#782]: https://github.com/stackabletech/operator-rs/pull/782

## [0.2.0] - 2024-03-26

### Changed

- Implement `PartialEq` for most _Snafu_ Error enums ([#757]).
- Update Rust to 1.77 ([#759]).

[#757]: https://github.com/stackabletech/operator-rs/pull/757
[#759]: https://github.com/stackabletech/operator-rs/pull/759
