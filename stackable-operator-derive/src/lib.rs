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
