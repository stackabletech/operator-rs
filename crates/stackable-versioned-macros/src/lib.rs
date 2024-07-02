use darling::{ast::NestedMeta, FromMeta};
use proc_macro::TokenStream;
use syn::{DeriveInput, Error};

use crate::attrs::common::ContainerAttributes;

mod attrs;
mod consts;
mod gen;

/// This macro enables generating versioned structs.
///
/// ## Usage Guide
///
/// ### Quickstart
///
/// ```
/// # use stackable_versioned_macros::versioned;
/// #[versioned(
///     version(name = "v1alpha1"),
///     version(name = "v1beta1"),
///     version(name = "v1"),
///     version(name = "v2"),
///     version(name = "v3")
/// )]
/// struct Foo {
///     /// My docs
///     #[versioned(
///         added(since = "v1beta1"),
///         renamed(since = "v1", from = "gau"),
///         deprecated(since = "v2", note = "not empty")
///     )]
///     deprecated_bar: usize,
///     baz: bool,
/// }
/// ```
///
/// ### Declaring Versions
///
/// Before any of the fields can be versioned, versions need to be declared at
/// the container level. Each version currently supports two parameters: `name`
/// and the `deprecated` flag. The `name` must be a valid (and supported)
/// format. The macro checks each declared version and reports any error
/// encountered during parsing.
/// The `deprecated` flag marks the version as deprecated. This currently adds
/// the `#[deprecated]` attribute to the appropriate piece of code.
///
/// ```
/// # use stackable_versioned_macros::versioned;
/// #[versioned(
///     version(name = "v1alpha1", deprecated)
/// )]
/// struct Foo {}
/// ```
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
/// struct Foo {}
/// ```
///
/// ### Field Actions
///
/// This library currently supports three different field actions. Fields can
/// be added, renamed and deprecated. The macro ensures that these actions
/// adhere to the following set of rules:
///
/// - Fields cannot be added and deprecated in the same version.
/// - Fields cannot be added and renamed in the same version.
/// - Fields cannot be renamed and deprecated in the same version.
/// - Fields added in version _a_, renamed _0...n_ times in versions
///   b<sub>1</sub>, b<sub>2</sub>, ..., b<sub>n</sub> and deprecated in
///   version _c_ must ensure _a < b<sub>1</sub>, b<sub>2</sub>, ...,
///   b<sub>n</sub> < c_.
/// - All field actions must use previously declared versions. Using versions
///   not present at the container level will result in an error.
///
/// For fields marked as deprecated, two additional rules apply:
///
/// - Fields must start with the `deprecated_` prefix.
/// - The deprecation note cannot be empty.
///
/// ### Auto-generated [`From`] Implementations
///
/// To enable smooth version upgrades of the same struct, the macro automatically
/// generates [`From`] implementations. On a high level, code generated for two
/// versions _a_ and _b_, with _a < b_ looks like this: `impl From<a> for b`.
///
/// ```ignore
/// #[versioned(
///     version(name = "v1alpha1"),
///     version(name = "v1beta1"),
///     version(name = "v1")
/// )]
/// pub struct Foo {
///     #[versioned(
///         added(since = "v1beta1"),
///         deprecated(since = "v1", note = "not needed")
///     )]
///     deprecated_bar: usize,
///     baz: bool,
/// }
///
/// // Produces ...
///
/// #[automatically_derived]
/// pub mod v1alpha1 {
///     pub struct Foo {
///         pub baz: bool,
///     }
/// }
/// #[automatically_derived]
/// #[allow(deprecated)]
/// impl From<v1alpha1::Foo> for v1beta1::Foo {
///     fn from(__sv_foo: v1alpha1::Foo) -> Self {
///         Self {
///             bar: std::default::Default::default(),
///             baz: __sv_foo.baz,
///         }
///     }
/// }
/// #[automatically_derived]
/// pub mod v1beta1 {
///     pub struct Foo {
///         pub bar: usize,
///         pub baz: bool,
///     }
/// }
/// #[automatically_derived]
/// #[allow(deprecated)]
/// impl From<v1beta1::Foo> for v1::Foo {
///     fn from(__sv_foo: v1beta1::Foo) -> Self {
///         Self {
///             deprecated_bar: __sv_foo.bar,
///             baz: __sv_foo.baz,
///         }
///     }
/// }
/// #[automatically_derived]
/// pub mod v1 {
///     pub struct Foo {
///         #[deprecated = "not needed"]
///         pub deprecated_bar: usize,
///         pub baz: bool,
///     }
/// }
/// ```
///
/// #### Skip [`From`] generation
///
/// Generation of these [`From`] implementations can be skipped at the container
/// and version level. This enables customization of the implementations if the
/// default implementation is not sufficient.
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
///         deprecated(since = "v1", note = "not needed")
///     )]
///     deprecated_bar: usize,
///     baz: bool,
/// }
/// ```
///
/// #### Customize Default Function for Added Fields
///
/// It is possible to customize the default function used in the generated
/// [`From`] implementation for populating added fields. By default,
/// [`Default::default()`] is used.
///
/// ```
/// # use stackable_versioned_macros::versioned;
/// #[versioned(
///     version(name = "v1alpha1"),
///     version(name = "v1beta1"),
///     version(name = "v1")
/// )]
/// pub struct Foo {
///     #[versioned(
///         added(since = "v1beta1", default = "default_bar"),
///         deprecated(since = "v1", note = "not needed")
///     )]
///     deprecated_bar: usize,
///     baz: bool,
/// }
///
/// fn default_bar() -> usize {
///     42
/// }
/// ```
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
    // because we only support structs (and eventually enums) to be versioned.
    // In the future - if we decide to support modules - this requires
    // adjustments to also support modules. One possible solution might be to
    // use an enum with two variants: Container(DeriveInput) and
    // Module(ItemMod).
    let input = syn::parse_macro_input!(input as DeriveInput);

    gen::expand(attrs, input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}
