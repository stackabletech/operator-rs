use syn::parse_macro_input;

mod fragment;
mod merge;

/// Derives [`Merge`](trait.Merge.html) for a given struct or enum, by merging each field individually.
///
/// For enums, all values of the previous variant are discarded if the variant is changed, even if the same field exists in both variants.
///
/// # Supported attributes
///
/// ## `#[merge(bound = "...")]`
///
/// This attribute can be used to specify additional `where` clauses on the derived trait implementation.
/// Bounds specified on the struct itself are automatically inherited for the generated implementation, and
/// do not need to be repeated here.
///
/// For example, this:
///
/// ```
/// # use stackable_operator::config::merge::Merge;
/// #[derive(Merge)]
/// #[merge(bound = "T: Merge")]
/// struct Wrapper<T> where T: Clone {
///     inner: T,
/// }
/// ```
///
/// Expands to (roughly) the following:
///
/// ```
/// # use stackable_operator::config::merge::Merge;
/// struct Wrapper<T> where T: Clone {
///     inner: T,
/// }
/// impl<T> Merge for Wrapper<T>
/// where
///     T: Clone, // this clause was inherited from the struct
///     T: Merge, // this clause was specified using #[merge(bound)]
/// {
///     fn merge(&mut self, defaults: &Self) {
///         self.inner.merge(&defaults.inner);
///     }
/// }
/// ```
///
/// ## `#[merge(path_overrides(merge = "..."))]`
///
/// This attribute can be used to override the path to the module containing the [`Merge`](trait.Merge.html) trait, if it is reexported
/// or the `stackable_operator` crate is renamed.
#[proc_macro_derive(Merge, attributes(merge))]
pub fn derive_merge(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    merge::derive(parse_macro_input!(input)).into()
}

/// Creates a [fragment type](index.html) for the given type, and implements [`FromFragment`](trait.FromFragment.html).
///
/// This macro implements "deep optionality", meaning that each field is replaced by its `Fragment` variant. For example, this type:
///
/// ```
/// #[derive(Fragment)]
/// struct Foo {
///     atomic: u32,
///     nested: Bar,
/// }
/// ```
///
/// would generate the following output:
///
/// ```
/// struct FooFragment {
///     atomic: u32::Fragment, // = Option<u32>
///     nested: Bar::Fragment, // = BarFragment, assuming that Bar is also #[derive(Fragment)]
/// }
///
/// impl FromFragment for Foo {
///     // no need for Option since the value is already deeply optional
///     type Fragment = FooFragment;
///     type RequiredFragment = FooFragment;
///
///     // snipped support code
/// }
/// ```
///
/// # Supported Attributes
///
/// ## `#[fragment_attrs(...)]`
///
/// This attribute can be used to forward attributes to the generated fragment. For example, `#[fragment_attrs(derive(Default))]` derives [`Default`] for the fragment type.
///
/// This can be specified on both the struct itself and the field, and will be forwarded to the corresponding location on the generated fragment.
///
/// ## `#[fragment(bound = "...")]`
///
/// This attribute can be used to specify additional `where` clauses on the derived fragment and trait implementation. Bounds specified on the struct itself
/// are automatically inherited for the generated implementation, and do not need to be repeated here.
///
/// ## `#[fragment(path_overrides(fragment = "..."))]`
///
/// This attribute can be used to override the path to the module containing the [`FromFragment`](trait.FromFragment.html) trait, if it is reexported
/// or the `stackable_operator` crate is renamed.
///
/// # Generics
///
/// This macro supports generic types, but there are some caveats to be aware of.
///
/// ## Fragment macro bounds
///
/// The `Fragment` macro does not automatically insert any type bounds, they must be spcified manually. Typically, this means adding the attribute
/// `#[fragment(bound = "T: FromFragment")]` to the type.
///
/// ## Interactions with other derive macros
///
/// Arbitrary other macros can be specified for the generated fragment type. However, many macros automatically add the bound `where T: Trait`. This assumption makes sense
/// for most types (with the assumption that each type is used "directly" in the struct definition).
///
/// However, fragment types use `T::Fragment` instead, and the generated bounds must be overridden to reflect this. The exact convention used to do this is going to vary
/// between macros, but typically they will take an attribute such as `#[trait(bound = "T::Fragment: Trait")]`.
///
/// ### `std` traits
///
/// `std`'s built-in derive macros (such as [`Default`]) do not take any configuration. Instead, use [`Derivative`](https://mcarton.github.io/rust-derivative/latest/index.html),
/// which supports custom bounds using the attribute convention `#[derivative(Trait(bound = "T::Fragment: Trait"))]`.
///
/// ### `serde`
///
/// Serde uses the `#[serde]` attribute to configure both `Serialize` and `Deserialize`. However, bounds must be configured separately for the two. Hence, the correct
/// bound would be:
///
/// ```
/// #[serde(bound(
///     serialize = "T::Fragment: Serialize",
///     deserialize = "T::Fragment: Deserialize<'de>",
/// ))]
/// ```
#[proc_macro_derive(Fragment, attributes(fragment, fragment_attrs))]
pub fn derive_fragment(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    fragment::derive(parse_macro_input!(input)).into()
}
