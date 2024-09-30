# Compile-Fail Testing

> [!NOTE]
> Also see the snapshot tests, described [here](../fixtures/README.md).

This type of testing is part of UI testing. These tests assert two things: First, the code should
**not** compile and secondly should also produce the expected rustc (compiler) error message. For
this type of testing, we use the [`trybuild`][trybuild] crate.

Tests are currently separated into two folders: `default` and `k8s`. The default test cases don't
require any additional features to be activated. The Kubernetes specific tests require the `k8s`
feature to be enabled. These tests can be run with `cargo test --all-features`.

Further information about the workflow are described [here][workflow].

[workflow]: https://docs.rs/trybuild/latest/trybuild/#workflow
[trybuild]: https://docs.rs/trybuild/latest/trybuild/
