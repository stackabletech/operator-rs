use darling::{ast::NestedMeta, FromMeta};
use proc_macro::TokenStream;
use syn::{spanned::Spanned, Error, Item};

use crate::{
    attrs::{container::StandaloneContainerAttributes, module::ModuleAttributes},
    codegen::{
        container::{Container, StandaloneContainer},
        module::{Module, ModuleInput},
        VersionDefinition,
    },
};

#[cfg(test)]
mod test_utils;

mod attrs;
mod codegen;
mod utils;

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
/// <div class="warning">
///
/// It is **important** to note that this macro must be placed before any other
/// (derive) macros and attributes. Macros supplied before the versioned macro
/// will be erased, because the original struct, enum or module (container) is
/// erased, and new containers are generated. This ensures that the macros and
/// attributes are applied to the generated versioned instances of the
/// container.
///
/// </div>
///
/// ## Declaring Versions
///
/// Before any of the fields or variants can be versioned, versions need to be
/// declared at the container level. Each version currently supports two
/// parameters: `name` and the `deprecated` flag. The `name` must be a valid
/// (and supported) format.
///
/// <div class="warning">
///
/// Currently, only Kubernetes API versions are supported. The macro checks each
/// declared version and reports any error encountered during parsing.
///
/// </div>
///
/// It should be noted that the defined struct always represents the **latest**
/// version, eg: when defining three versions `v1alpha1`, `v1beta1`, and `v1`,
/// the struct will describe the structure of the data in `v1`. This behaviour
/// is especially noticeable in the [`changed()`](#changed-action) action which
/// works "backwards" by describing how a field looked before the current
/// (latest) version.
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
/// ## Versioning Items in a Module
///
/// Using the macro on structs and enums is explained in detail in the following
/// sections. This section is dedicated to explain the usage of the macro when
/// applied to a module.
///
/// Using the macro on a module has one clear use-case: Versioning multiple
/// structs and enums at once in **a single file**. Applying the `#[versioned]`
/// macro to individual containers will result in invalid Rust code which the
/// compiler rejects. This behaviour can best be explained using the following
/// example:
///
/// ```ignore
/// # use stackable_versioned_macros::versioned;
/// #[versioned(version(name = "v1alpha1"))]
/// struct Foo {}
///
/// #[versioned(version(name = "v1alpha1"))]
/// struct Bar {}
/// ```
///
/// In this example, two different structs are versioned using the same version,
/// `v1alpha1`. Each macro will now (independently) expand into versioned code.
/// This will result in the module named `v1alpha1` to be emitted twice, in the
/// same file. This is invalid Rust code. You cannot define the same module more
/// than once in the same file.
///
/// <details>
/// <summary>Expand Generated Invalid Code</summary>
///
/// ```ignore
/// mod v1alpha1 {
///     struct Foo {}
/// }
///
/// mod v1alpha1 {
///     struct Bar {}
/// }
/// ```
/// </details>
///
/// This behaviour makes it impossible to version multiple containers in the
/// same file. The only solution would be to put each container into its own
/// file which in many cases is not needed or even undesired. To solve this
/// issue, it is thus possible to apply the macro to a module.
///
/// ```
/// # use stackable_versioned_macros::versioned;
/// #[versioned(
///     version(name = "v1alpha1"),
///     version(name = "v1")
/// )]
/// mod versioned {
///     struct Foo {
///         bar: usize,
///     }
///
///     struct Bar {
///         baz: String,
///     }
/// }
/// ```
///
/// <details>
/// <summary>Expand Generated Code</summary>
///
/// 1. All containers defined in the module will get versioned. That's why every
///    version module includes all containers.
/// 2. Each version will expand to a version module, as expected.
///
/// ```ignore
/// mod v1alpha1 {
///     use super::*;
///     pub struct Foo { // 1
///         bar: usize,
///     }
///     pub struct Bar { // 1
///         baz: String,
///     }
/// }
///
/// mod v1 {             // 2
///     use super::*;
///     pub struct Foo {
///         bar: usize,
///     }
///     pub struct Bar {
///         baz: String,
///     }
/// }
/// ```
/// </details>
///
/// It should be noted that versions are now defined at the module level and
/// **not** at the struct / enum level. Item actions describes in the following
/// section can be used as expected.
///
/// ### Preserve Module
///
/// The previous examples completely replaced the `versioned` module with
/// top-level version modules. This is the default behaviour. Preserving the
/// module can however be enabled by setting the `preserve_module` flag.
///
/// ```
/// # use stackable_versioned_macros::versioned;
/// #[versioned(
///     version(name = "v1alpha1"),
///     version(name = "v1"),
///     options(preserve_module)
/// )]
/// mod versioned {
///     struct Foo {
///         bar: usize,
///     }
///
///     struct Bar {
///         baz: String,
///     }
/// }
/// ```
///
/// <div class="warning">
/// It is planned to move the <code>preserve_module</code> flag into the
/// <code>options()</code> argument list, but currently seems tricky to
/// implement.
/// </div>
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
/// <summary>Expand Generated Code</summary>
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
/// <summary>Expand Generated Code</summary>
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
/// <summary>Expand Generated Code</summary>
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
/// <summary>Expand Generated Code</summary>
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
#[cfg_attr(
    feature = "k8s",
    doc = r#"
## Kubernetes-specific Features

This macro also offers support for Kubernetes-specific versioning,
especially for CustomResourceDefinitions (CRDs). These features are
completely opt-in. You need to enable the `k8s` feature (which enables
optional dependencies) and use the `k8s()` parameter in the macro.

You need to derive both [`kube::CustomResource`] and [`schemars::JsonSchema`].

```
# use stackable_versioned_macros::versioned;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1"),
    k8s(group = "example.com")
)]
#[derive(Clone, Debug, Deserialize, Serialize, CustomResource, JsonSchema)]
pub struct FooSpec {
    #[versioned(
        added(since = "v1beta1"),
        changed(since = "v1", from_name = "prev_bar", from_type = "u16")
    )]
    bar: usize,
    baz: bool,
}

# fn main() {
let merged_crd = Foo::merged_crd(Foo::V1).unwrap();
println!("{}", serde_yaml::to_string(&merged_crd).unwrap());
# }
```

The generated `merged_crd` method is a wrapper around [kube's `merge_crds`][1]
function. It automatically calls the `crd` methods of the CRD in all of its
versions and additionally provides a strongly typed selector for the stored
API version.

Currently, the following arguments are supported:

- `group`: Set the group of the CR object, usually the domain of the company.
  This argument is Required.
- `kind`: Override the kind field of the CR object. This defaults to the struct
   name (without the 'Spec' suffix).
- `singular`: Set the singular name of the CR object.
- `plural`: Set the plural name of the CR object.
- `namespaced`: Indicate that this is a namespaced scoped resource rather than a
   cluster scoped resource.
- `crates`: Override specific crates.
- `status`: Set the specified struct as the status subresource.
- `shortname`: Set a shortname for the CR object. This can be specified multiple
  times.

### Versioning Items in a Module

Versioning multiple CRD related structs via a module is supported and common
rules from [above](#versioning-items-in-a-module) apply here as well. It should
however be noted, that specifying Kubernetes specific arguments is done on the
container level instead of on the module level, which is detailed in the
following example:

```
# use stackable_versioned_macros::versioned;
# use kube::CustomResource;
# use schemars::JsonSchema;
# use serde::{Deserialize, Serialize};
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1")
)]
mod versioned {
    #[versioned(k8s(group = "foo.example.org"))]
    #[derive(Clone, Debug, Deserialize, Serialize, CustomResource, JsonSchema)]
    struct FooSpec {
        bar: usize,
    }

    #[versioned(k8s(group = "bar.example.org"))]
    #[derive(Clone, Debug, Deserialize, Serialize, CustomResource, JsonSchema)]
    struct BarSpec {
        baz: String,
    }
}

# fn main() {
let merged_crd = Foo::merged_crd(Foo::V1).unwrap();
println!("{}", serde_yaml::to_string(&merged_crd).unwrap());
# }
```

<details>
<summary>Expand Generated Code</summary>

```ignore
mod v1alpha1 {
    use super::*;
    #[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, CustomResource)]
    #[kube(
        group = "foo.example.org",
        version = "v1alpha1",
        kind = "Foo"
    )]
    pub struct FooSpec {
        pub bar: usize,
    }

    #[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, CustomResource)]
    #[kube(
        group = "bar.example.org",
        version = "v1alpha1",
        kind = "Bar"
    )]
    pub struct BarSpec {
        pub bar: usize,
    }
}

// Automatic From implementations for conversion between versions ...

mod v1 {
    use super::*;
    #[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, CustomResource)]
    #[kube(
        group = "foo.example.org",
        version = "v1",
        kind = "Foo"
    )]
    pub struct FooSpec {
        pub bar: usize,
    }

    #[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, CustomResource)]
    #[kube(
        group = "bar.example.org",
        version = "v1",
        kind = "Bar"
    )]
    pub struct BarSpec {
        pub bar: usize,
    }
}

// Implementations to create the merged CRDs ...
```
</details>

It is possible to include structs and enums which are not CRDs. They are instead
versioned as expected (without adding the `#[kube]` derive macro and generating
code to merge CRD versions).
"#
)]
#[proc_macro_attribute]
pub fn versioned(attrs: TokenStream, input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as Item);
    versioned_impl(attrs.into(), input).into()
}

fn versioned_impl(attrs: proc_macro2::TokenStream, input: Item) -> proc_macro2::TokenStream {
    // TODO (@Techassi): Think about how we can handle nested structs / enums which
    // are also versioned.

    match input {
        Item::Mod(item_mod) => {
            let module_attributes: ModuleAttributes = match parse_outer_attributes(attrs) {
                Ok(ma) => ma,
                Err(err) => return err.write_errors(),
            };

            let versions: Vec<VersionDefinition> = (&module_attributes).into();
            let preserve_modules = module_attributes
                .common
                .options
                .preserve_module
                .is_present();

            let skip_from = module_attributes
                .common
                .options
                .skip
                .as_ref()
                .map_or(false, |opts| opts.from.is_present());

            let module_span = item_mod.span();
            let module_input = ModuleInput {
                ident: item_mod.ident,
                vis: item_mod.vis,
            };

            let Some((_, items)) = item_mod.content else {
                return Error::new(module_span, "the macro can only be used on module blocks")
                    .into_compile_error();
            };

            let mut containers = Vec::new();

            for item in items {
                let container = match item {
                    Item::Enum(item_enum) => {
                        match Container::new_enum_nested(item_enum, &versions) {
                            Ok(container) => container,
                            Err(err) => return err.write_errors(),
                        }
                    }
                    Item::Struct(item_struct) => {
                        match Container::new_struct_nested(item_struct, &versions) {
                            Ok(container) => container,
                            Err(err) => return err.write_errors(),
                        }
                    }
                    _ => continue,
                };

                containers.push(container);
            }

            Module::new(
                module_input,
                preserve_modules,
                skip_from,
                versions,
                containers,
            )
            .generate_tokens()
        }
        Item::Enum(item_enum) => {
            let container_attributes: StandaloneContainerAttributes =
                match parse_outer_attributes(attrs) {
                    Ok(ca) => ca,
                    Err(err) => return err.write_errors(),
                };

            let standalone_enum =
                match StandaloneContainer::new_enum(item_enum, container_attributes) {
                    Ok(standalone_enum) => standalone_enum,
                    Err(err) => return err.write_errors(),
                };

            standalone_enum.generate_tokens()
        }
        Item::Struct(item_struct) => {
            let container_attributes: StandaloneContainerAttributes =
                match parse_outer_attributes(attrs) {
                    Ok(ca) => ca,
                    Err(err) => return err.write_errors(),
                };

            let standalone_struct =
                match StandaloneContainer::new_struct(item_struct, container_attributes) {
                    Ok(standalone_struct) => standalone_struct,
                    Err(err) => return err.write_errors(),
                };

            standalone_struct.generate_tokens()
        }
        _ => Error::new(
            input.span(),
            "attribute macro `versioned` can be only be applied to modules, structs and enums",
        )
        .into_compile_error(),
    }
}

fn parse_outer_attributes<T>(attrs: proc_macro2::TokenStream) -> Result<T, darling::Error>
where
    T: FromMeta,
{
    let nm = NestedMeta::parse_meta_list(attrs)?;
    T::from_list(&nm)
}

#[cfg(test)]
mod test {
    use insta::{assert_snapshot, glob};

    use super::*;

    #[test]
    fn default_snapshots() {
        let _settings_guard = test_utils::set_snapshot_path().bind_to_scope();

        glob!("../fixtures/inputs/default", "*.rs", |path| {
            let formatted = test_utils::expand_from_file(path)
                .inspect_err(|err| eprintln!("{err}"))
                .unwrap();
            assert_snapshot!(formatted);
        });
    }

    #[cfg(feature = "k8s")]
    #[test]
    fn k8s_snapshots() {
        let _settings_guard = test_utils::set_snapshot_path().bind_to_scope();

        glob!("../fixtures/inputs/k8s", "*.rs", |path| {
            let formatted = test_utils::expand_from_file(path)
                .inspect_err(|err| eprintln!("{err}"))
                .unwrap();
            assert_snapshot!(formatted);
        });
    }
}
