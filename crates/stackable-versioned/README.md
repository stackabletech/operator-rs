<!-- markdownlint-disable MD041 MD033 -->

<p align="center">
  <img width="150" src="../../.readme/static/borrowed/Icon_Stackable.svg" alt="Stackable Logo"/>
</p>

<h1 align="center">stackable-versioned</h1>

[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-green.svg)](https://docs.stackable.tech/home/stable/contributor/index.html)
[![Apache License 2.0](https://img.shields.io/badge/license-Apache--2.0-green)](./LICENSE)

[Stackable Data Platform](https://stackable.tech/) | [Platform Docs](https://docs.stackable.tech/) | [Discussions](https://github.com/orgs/stackabletech/discussions) | [Discord](https://discord.gg/7kZ3BNnCAF)

This crate enables versioning of structs (and enums in the future). It currently
supports Kubernetes API versions while declaring versions on a data type. This
will be extended to support SemVer versions, as well as custom version formats
in the future.

```rust
use stackable_versioned::versioned;

#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1"),
    version(name = "v2"),
    version(name = "v3")
)]
struct Foo {
    /// My docs
    #[versioned(
        added(since = "v1alpha1"),
        renamed(since = "v1beta1", from = "gau"),
        deprecated(since = "v2", note = "not required anymore")
    )]
    deprecated_bar: usize,
    baz: bool,
}
```
