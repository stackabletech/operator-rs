mod merge;
mod optional;

/// Derives [`Merge`](trait.Merge.html) for a given struct or enum, by merging each field individually.
///
/// For enums, all values of the previous variant are discarded if the variant is changed, even if the same field exists in both variants.
///
/// # Supported attributes
///
/// ## `#[merge(bounds = "...")]`
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
/// #[merge(bounds = "T: Merge")]
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
///     T: Merge, // this clause was specified using #[merge(bounds)]
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
    merge::derive(input)
}

/// Derives an [`Optional`](trait.Optional.html) trait for a given struct. The [`Optional`] trait
/// expands to a copy of the original struct with all fields transformed to Options and the name
/// prefixed with `Optional`. A defined struct called `FooConfig` would expand to a struct called
/// `OptionalFooConfig`. Default values or methods can be provided for each field.
///
/// Furthermore an implementation for `From<OptionalFooConfig>` is generated for `FooConfig` to
/// retrieve back the original `FooConfig` struct.
///
/// The generated struct is intended to be used in the operators CRD (e.g. `OptionalFooConfig`)
/// as well as in the merging process of role configs and role group configs.
///
/// The generated struct `OptionalFooConfig` derives the `Merge` trait.
///
/// # Supported field attributes
///
/// ## `#[optional(default_value = "...")]`
///
/// This attribute can be used to provide a default value to fall back on if the optional value in
/// the generated struct is not set
///
/// ## `#[optional(default_impl = "...")]`
///
/// This attribute can be used to provide a default implementation to fall back on if the optional
/// value in the generated struct is not set
///
/// # Example
/// ```
/// # use stackable_operator::config::optional::Optional;
/// const DEFAULT_PORT: u16 = 11111;
/// // For example, this:
/// #[derive(Optional)]
/// struct FooConfig {
///     #[optional(default = "DEFAULT_PORT")]
///     port: u16,
/// }
/// // Expands to (roughly) the following:
/// #[derive(Merge)]
/// struct OptionalFooConfig {
///     port: Option<u16>,
/// }
/// impl From<OptionalFooConfig> for FooConfig {
///    fn from(c: OptionalFooConfig) -> Self {
///        Self {
///            port: c.port.unwrap_or(DEFAULT_PORT),
///        }
///    }
/// }
/// impl Optional for OptionalFooConfig {}
/// ```
#[proc_macro_derive(Optional, attributes(optional))]
pub fn derive_optional(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    optional::derive(input)
}
