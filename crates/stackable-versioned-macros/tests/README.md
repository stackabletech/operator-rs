# Testing the `#[versioned]` macro

## Snapshot Testing

> [!NOTE]
> Please have `rust-src` installed, e.g. using `rustup component add rust-src`.
>
> Also see the compile-fail tests, described [here](../tests/README.md).

This folder contains fixtures for snapshot testing the `#[versioned()]` macro. Snapshot testing is
done using the [insta] crate. It provides a [CLI tool][insta-cli] called `cargo-insta` and a
[VS Code extension][insta-ext].

Test inputs and snapshots of the expected output are located in the `fixtures` folder. There are two
inputs to the `#[versioned()]` macro because it is an attribute macro:

> The first TokenStream is the delimited token tree following the attribute’s name, not including
> the outer delimiters. If the attribute is written as a bare attribute name, the attribute
> TokenStream is empty. The second TokenStream is the rest of the item including other attributes on
> the item.
>
> _(Taken from the [Rust reference][rust-ref])_

Because of that, a special delimiter is used in the input files which separates the two inputs while
still enabling developers to write valid Rust code. The delimiter is `// ---\n`. Most of the inner
workings are located in [this file](../src/test_utils.rs).

```rust
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1")
)]
// --- <- See here!
pub(crate) struct Foo {
    #[versioned(
        changed(since = "v1beta1", from_name = "jjj", from_type = "u8"),
        changed(since = "v1", from_type = "u16"),
    )]
    bar: usize,
    baz: bool,
}
```

### Recommended Workflow

First, add new input files (which automatically get picked up by `insta`) to the `fixtures/inputs`
folder. Make sure the delimiter is placed correctly between the attribute and the container
definition. Doc comments on the container have to be placed after the delimiter. Next, generate the
snapshot files (initially not accepted) by running

```shell
cargo insta test -p stackable-versioned-macros
```

This command will place the new snapshot files (with a `.new` extension) in the `fixtures/snapshots`
folder. These new snapshot files must not appear on `main`, but can be shared on branches for
collaboration. To review them, run the `cargo insta review` command, then accept or fix the
snapshots. Once all are accepted (ie: no `.new` files remaining), check in the files.

## Compile-Fail Testing

> [!NOTE]
> Also see the snapshot tests, described [here](../fixtures/README.md).

This type of testing is part of UI testing. These tests assert two things: First, the code should
**not** compile and secondly should also produce the expected rustc (compiler) error message. For
this type of testing, we use the [`trybuild`][trybuild] crate.

Tests are currently separated into two folders: `default` and `k8s`. The default test cases don't
require any additional features to be activated. The Kubernetes specific tests require the `k8s`
feature to be enabled. These tests can be run with `cargo test --all-features`.

Further information about the workflow are described [here][workflow].

[rust-ref]: https://doc.rust-lang.org/reference/procedural-macros.html#attribute-macros
[workflow]: https://docs.rs/trybuild/latest/trybuild/#workflow
[trybuild]: https://docs.rs/trybuild/latest/trybuild/
[insta-ext]: https://insta.rs/docs/vscode/
[insta-cli]: https://insta.rs/docs/cli/
[insta]: https://insta.rs/
