# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- `DeletionPolicy`, whether an agent deletes a resource in the product ([#1290]).
- `ResourceOperation` with `reconciliationPaused`, which stops an agent from changing a resource
  in the product ([#1290]).
- `Ready`/`Synced` conditions of resources managed by agents (`condition`), as `metav1.Condition`
  with `True`/`False`/`Unknown` and `observedGeneration` ([#1290]).

## [0.0.1]

### Added

- Add the `stackable-agent` crate as a workspace member ([#1290]).

[#1290]: https://github.com/stackabletech/operator-rs/pull/1290
