use darling::{ast::NestedMeta, FromMeta};
use proc_macro::TokenStream;
use syn::{DeriveInput, Error};

use crate::attrs::common::ContainerAttributes;

mod attrs;
mod codegen;
mod consts;

/// This macro enables generating versioned structs and enums.
///
/// # Usage Guide
///
/// In this guide, code blocks usually come in pairs. The first code block
/// describes how the macro is used. The second expandable block displays the
/// generated piece of code for explanation purposes. It should be noted, that
/// the exact code can diverge from what is being depicted in this guide. For
/// example, `#[automatically_derived]` and `#[allow(deprecated)]` are removed
/// in most examples to reduce visual clutter.
///
/// ## Declaring Versions
///
/// It is **important** to note that this macro must be placed before any other
/// (derive) macros and attributes. Macros supplied before the versioned macro
/// will be erased, because the original struct or enum (container) is erased,
/// and new containers are generated. This ensures that the macros and
/// attributes are applied to the generated versioned instances of the
/// container.
///
/// Before any of the fields or variants can be versioned, versions need to be
/// declared at the container level. Each version currently supports two
/// parameters: `name` and the `deprecated` flag. The `name` must be a valid
/// (and supported) format.
///
/// <div class="warning">
/// Currently, only Kubernetes API versions are supported. The macro checks each
/// declared version and reports any error encountered during parsing.
/// </div>
///
/// ```
/// # use stackable_versioned_macros::versioned;
/// #[versioned(version(name = "v1alpha1"))]
/// struct Foo {
///     bar: usize,
/// }
/// ```
///
/// <details>
/// <summary>Generated code</summary>
///
/// 1. The `#[automatically_derived]` attribute indicates that the following
///    piece of code is automatically generated by a macro instead of being
///    handwritten by a developer. This information is used by cargo and rustc.
/// 2. For each declared version, a new module containing the container is
///    generated. This enables you to reference the container by versions via
///    `v1alpha1::Foo`.
/// 3. This `use` statement gives the generated containers access to the imports
///    at the top of the file. This is a convenience, because otherwise you
///    would need to prefix used items with `super::`. Additionally, other
///    macros can have trouble using items referred to with `super::`.
///
/// ```ignore
/// #[automatically_derived] // 1
/// mod v1alpha1 {           // 2
///     use super::*;        // 3
///     pub struct Foo {
///         bar: usize,
///     }
/// }
/// ```
/// </details>
///
/// ### Deprecation of a Version
///
/// The `deprecated` flag marks the version as deprecated. This currently adds
/// the `#[deprecated]` attribute to the appropriate piece of code.
///
/// ```
/// # use stackable_versioned_macros::versioned;
/// #[versioned(version(name = "v1alpha1", deprecated))]
/// struct Foo {
///     bar: usize,
/// }
/// ```
///
/// <details>
/// <summary>Generated code</summary>
///
/// 1. The `deprecated` flag will generate a `#[deprecated]` attribute and the
///    note is automatically generated.
///
/// ```ignore
/// #[automatically_derived]
/// #[deprecated = "Version v1alpha1 is deprecated"] // 1
/// mod v1alpha1 {
///     use super::*;
///     pub struct Foo {
///         pub bar: usize,
///     }
/// }
/// ```
/// </details>
///
/// ### Version Sorting
///
/// Additionally, it is ensured that each version is unique. Declaring the same
/// version multiple times will result in an error. Furthermore, declaring the
/// versions out-of-order is prohibited by default. It is possible to opt-out
/// of this check by setting `options(allow_unsorted)`:
///
/// ```
/// # use stackable_versioned_macros::versioned;
/// #[versioned(
///     version(name = "v1beta1"),
///     version(name = "v1alpha1"),
///     options(allow_unsorted)
/// )]
/// struct Foo {
///     bar: usize,
/// }
/// ```
///
/// ## Item Actions
///
/// This crate currently supports three different item actions. Items can
/// be added, changed, and deprecated. The macro ensures that these actions
/// adhere to the following set of rules:
///
/// 1. Items cannot be added and deprecated in the same version.
/// 2. Items cannot be added and changed in the same version.
/// 3. Items cannot be changed and deprecated in the same version.
/// 4. Items added in version _a_, renamed _0...n_ times in versions
///    b<sub>1</sub>, ..., b<sub>n</sub> and deprecated in
///    version _c_ must ensure _a < b<sub>1</sub>, ..., b<sub>n</sub> < c_.
/// 5. All item actions must use previously declared versions. Using versions
///    not present at the container level will result in an error.
///
/// For items marked as deprecated, one additional rule applies:
///
/// - Fields must start with the `deprecated_` and variants with the
///   `Deprecated` prefix. This is enforced because Kubernetes doesn't allow
///   removing fields in CRDs entirely. Instead, they should be marked as
///   deprecated. By convention this is done with the `deprecated` prefix.
///
/// ### Added Action
///
/// This action indicates that an item is added in a particular version.
/// Available parameters are:
///
/// - `since` to indicate since which version the item is present.
/// - `default` to customize the default function used to populate the item
///   in auto-generated [`From`] implementations.
///
/// ```
/// # use stackable_versioned_macros::versioned;
/// #[versioned(
///     version(name = "v1alpha1"),
///     version(name = "v1beta1")
/// )]
/// pub struct Foo {
///     #[versioned(added(since = "v1beta1"))]
///     bar: usize,
///     baz: bool,
/// }
/// ```
///
/// <details>
/// <summary>Generated code</summary>
///
/// 1. The field `bar` is not yet present in version `v1alpha1` and is therefore
///    not generated.
/// 2. Now the field `bar` is present and uses `Default::default()` to populate
///    the field during conversion. This function can be customized as shown
///    later in this guide.
///
/// ```ignore
/// pub mod v1alpha1 {
///     use super::*;
///     pub struct Foo {                 // 1
///         pub baz: bool,
///     }
/// }
///
/// impl From<v1alpha1::Foo> for v1beta1::Foo {
///     fn from(foo: v1alpha1::Foo) -> Self {
///         Self {
///             bar: Default::default(), // 2
///             baz: foo.baz,
///         }
///     }
/// }
///
/// pub mod v1beta1 {
///     use super::*;
///     pub struct Foo {
///         pub bar: usize,              // 2
///         pub baz: bool,
///     }
/// }
/// ```
/// </details>
///
/// #### Custom Default Function
///
/// To customize the default function used in the generated `From` implementation
/// you can use the `default` parameter. It expects a path to a function without
/// braces.
///
/// ```
/// # use stackable_versioned_macros::versioned;
/// #[versioned(
///     version(name = "v1alpha1"),
///     version(name = "v1beta1")
/// )]
/// pub struct Foo {
///     #[versioned(added(since = "v1beta1", default = "default_bar"))]
///     bar: usize,
///     baz: bool,
/// }
///
/// fn default_bar() -> usize {
///     42
/// }
/// ```
///
/// <details>
/// <summary>Generated code</summary>
///
/// 1. Instead of `Default::default()`, the provided function `default_bar()` is
///    used. It is of course fully type checked and needs to return the expected
///    type (`usize` in this case).
///
/// ```ignore
/// // Snip
///
/// impl From<v1alpha1::Foo> for v1beta1::Foo {
///     fn from(foo: v1alpha1::Foo) -> Self {
///         Self {
///             bar: default_bar(), // 1
///             baz: foo.baz,
///         }
///     }
/// }
///
/// // Snip
/// ```
/// </details>
///
/// ### Changed Action
///
/// This action indicates that an item is changed in a particular version. It
/// combines renames and type changes into a single action. You can choose to
/// change the name, change the type or do both. Available parameters are:
///
/// - `since` to indicate since which version the item is changed.
/// - `from_name` to indicate from which previous name the field is renamed.
/// - `from_type` to indicate from which previous type the field is changed.
///
/// ```
/// # use stackable_versioned_macros::versioned;
/// #[versioned(
///     version(name = "v1alpha1"),
///     version(name = "v1beta1")
/// )]
/// pub struct Foo {
///     #[versioned(changed(
///         since = "v1beta1",
///         from_name = "prev_bar",
///         from_type = "u16"
///     ))]
///     bar: usize,
///     baz: bool,
/// }
/// ```
///
/// <details>
/// <summary>Generated code</summary>
///
/// 1. In version `v1alpha1` the field is named `prev_bar` and uses a `u16`.
/// 2. In the next version, `v1beta1`, the field is now named `bar` and uses
///    `usize` instead of a `u16`. The `From` implementation transforms the
///     type automatically via the `.into()` call.
///
/// ```ignore
/// pub mod v1alpha1 {
///     use super::*;
///     pub struct Foo {
///         pub prev_bar: u16,            // 1
///         pub baz: bool,
///     }
/// }
///
/// impl From<v1alpha1::Foo> for v1beta1::Foo {
///     fn from(foo: v1alpha1::Foo) -> Self {
///         Self {
///             bar: foo.prev_bar.into(), // 2
///             baz: foo.baz,
///         }
///     }
/// }
///
/// pub mod v1beta1 {
///     use super::*;
///     pub struct Foo {
///         pub bar: usize,               // 2
///         pub baz: bool,
///     }
/// }
/// ```
/// </details>
///
/// ### Deprecated Action
///
/// This action indicates that an item is deprecated in a particular version.
/// Deprecated items are not removed.
///
/// ```
/// # use stackable_versioned_macros::versioned;
/// #[versioned(version(name = "v1alpha1"), version(name = "v1beta1"))]
/// pub struct Foo {
///     #[versioned(deprecated(since = "v1beta1"))]
///     deprecated_bar: usize,
///     baz: bool,
/// }
/// ```
///
/// <details>
/// <summary>Generated code</summary>
///
/// 1. In version `v1alpha1` the field `bar` is not yet deprecated and thus uses
///    the name without the `deprecated_` prefix.
/// 2. In version `v1beta1` the field is deprecated and now includes the
///    `deprecated_` prefix. It also uses the `#[deprecated]` attribute to
///    indicate to Clippy this part of Rust code is deprecated. Therefore, the
///    `From` implementation includes `#[allow(deprecated)]` to allow the
///    usage of deprecated items in automatically generated code.
///
/// ```ignore
/// pub mod v1alpha1 {
///     use super::*;
///     pub struct Foo {
///         pub bar: usize,                     // 1
///         pub baz: bool,
///     }
/// }
///
/// #[allow(deprecated)]                        // 2
/// impl From<v1alpha1::Foo> for v1beta1::Foo {
///     fn from(foo: v1alpha1::Foo) -> Self {
///         Self {
///             deprecated_bar: foo.bar,        // 2
///             baz: foo.baz,
///         }
///     }
/// }
///
/// pub mod v1beta1 {
///     use super::*;
///     pub struct Foo {
///         #[deprecated]                       // 2
///         pub deprecated_bar: usize,
///         pub baz: bool,
///     }
/// }
/// ```
/// </details>
///
/// ## Auto-generated `From` Implementations
///
/// To enable smooth container version upgrades, the macro automatically
/// generates `From` implementations. On a high level, code generated for two
/// versions _a_ and _b_, with _a < b_ looks like this: `impl From<a> for b`.
/// As you can see, only upgrading is currently supported. Downgrading from a
/// higher version to a lower one is not supported at the moment.
///
/// This automatic generation can be skipped to enable a custom implementation
/// for more complex conversions.
///
/// ### Skipping at the Container Level
///
/// Disabling this behavior at the container level results in no `From`
/// implementation for all versions.
///
/// ```
/// # use stackable_versioned_macros::versioned;
/// #[versioned(
///     version(name = "v1alpha1"),
///     version(name = "v1beta1"),
///     version(name = "v1"),
///     options(skip(from))
/// )]
/// pub struct Foo {
///     #[versioned(
///         added(since = "v1beta1"),
///         deprecated(since = "v1")
///     )]
///     deprecated_bar: usize,
///     baz: bool,
/// }
/// ```
///
/// ### Skipping at the Version Level
///
/// Disabling this behavior at the version level results in no `From`
/// implementation for that particular version. This can be read as "skip
/// generation for converting _this_ version to the next one". In the example
/// below no conversion between version `v1beta1` and `v1` is generated.
///
/// ```
/// # use stackable_versioned_macros::versioned;
/// #[versioned(
///     version(name = "v1alpha1"),
///     version(name = "v1beta1", skip(from)),
///     version(name = "v1")
/// )]
/// pub struct Foo {
///     #[versioned(
///         added(since = "v1beta1"),
///         deprecated(since = "v1")
///     )]
///     deprecated_bar: usize,
///     baz: bool,
/// }
/// ```
///
/// ## Kubernetes-specific Features
///
/// This macro also offers support for Kubernetes-specific versioning,
/// especially for CustomResourceDefinitions (CRDs). These features are
/// completely opt-in. You need to enable the `k8s` feature (which enables
/// optional dependencies) and use the `k8s()` parameter in the macro.
///
#[cfg_attr(
    feature = "k8s",
    doc = r#"
```
# use stackable_versioned_macros::versioned;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1"),
    k8s(group = "example.com")
)]
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct FooSpec {
    #[versioned(
        added(since = "v1beta1"),
        changed(since = "v1", from_name = "prev_bar", from_type = "u16")
    )]
    bar: usize,
    baz: bool,
}
let merged_crd = Foo::merged_crd("v1").unwrap();
println!("{}", serde_yaml::to_string(&merged_crd).unwrap());
```
"#
)]
#[proc_macro_attribute]
pub fn versioned(attrs: TokenStream, input: TokenStream) -> TokenStream {
    let attrs = match NestedMeta::parse_meta_list(attrs.into()) {
        Ok(attrs) => match ContainerAttributes::from_list(&attrs) {
            Ok(attrs) => attrs,
            Err(err) => return err.write_errors().into(),
        },
        Err(err) => return darling::Error::from(err).write_errors().into(),
    };

    // NOTE (@Techassi): For now, we can just use the DeriveInput type here,
    // because we only support structs end enums to be versioned.
    // In the future - if we decide to support modules - this requires
    // adjustments to also support modules. One possible solution might be to
    // use an enum with two variants: Container(DeriveInput) and
    // Module(ItemMod).
    let input = syn::parse_macro_input!(input as DeriveInput);

    codegen::expand(attrs, input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}
