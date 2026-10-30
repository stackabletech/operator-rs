# Changelog

All notable changes to this project will be documented in this file.

## [0.1.0]

### Added

- `DeletionPolicy`, whether an agent deletes a resource in the product.
- `ResourceOperation` with `reconciliationPaused`, which stops an agent from changing a resource
  in the product.
- `Ready`/`Synced` conditions of resources managed by agents (`condition`), as `metav1.Condition`
  with `True`/`False`/`Unknown` and `observedGeneration`.
