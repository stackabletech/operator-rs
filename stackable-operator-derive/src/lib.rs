mod config;
mod merge;

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

/// Derives a [`Config`](trait.Config.html) trait for a given struct. The [`Config`] trait expands to
/// a copy of the original struct with all fields transformed to Options. Furthermore an implementation
/// to retrieve the back the derived struct is generated.
///
/// The generated struct is intended to be used in the operators CRD (e.g. FooConfig) as well as
/// in the merging process of role configs and role group configs.
///
/// # Supported field attributes
///
/// ## `#[config(default_value = "...")]`
///
/// This attribute can be used to provide a default value to fall back on if the optional value in
/// the generated struct is not set
///
/// ## `#[config(default_impl = "...")]`
///
/// This attribute can be used to provide a default implementation to fall back on if the optional
/// value in the generated struct is not set
///
/// # Example
/// ```
/// # use stackable_operator::config::config::Config;
/// const DEFAULT_PORT: u16 = 11111;
/// // For example, this:
/// #[derive(Config)]
/// struct FooConfig {
///     #[config(default = "DEFAULT_PORT")]
///     port: u16,
/// }
/// // Expands to roughly the following:
/// #[derive(Merge)]
/// struct MergableFooConfig {
///     port: Option<u16>,
/// }
/// impl From<MergableFooConfig> for FooConfig {
///    fn from(c: MergableFooConfig) -> Self {
///        Self {
///            port: c.port.unwrap_or(DEFAULT_PORT),
///        }
///    }
///}
/// ```
#[proc_macro_derive(Config, attributes(config))]
pub fn derive_config(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    config::derive(input)
}
