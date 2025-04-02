//! The Stacklet Configuration System™©®️️️ (SCS).
//!
//! # But oh god why is this monstrosity a thing?
//!
//! Products are complicated. They need to be supplied many kinds of configuration.
//! Some of it applies to the whole installation (Stacklet). Some of it applies only to one [role](`Role`).
//! Some of it applies only to a subset of the instances of that role (we call this a [`RoleGroup`]).
//!
//! We (usually) don't know at what level it makes sense to apply a given piece of configuration, but we also
//! don't want to force users to repeat themselves constantly! Instead, we model the configuration as a tree:
//!
//! ```yaml
//! stacklet1:
//!   role1:
//!     group1:
//!     group2:
//!   role2:
//!     group3:
//!     group4:
//! stacklet2:
//!   role3:
//!     group5:
//! ```
//!
//! Where only the leaves (*groups*) are actually realized into running products, but every level inherits
//! the configuration of its parents. So `group1` would inherit any keys from `role1` (and, transitively, `stacklet1`),
//! unless it overrides them.
//!
//! We also want to *validate* that the configuration actually makes sense, but only once we have the fully realized
//! configuration for a given rolegroup.
//!
//! However, in practice, living in a fully typed land like Rust makes this slightly awkward. We end up having to choose from
//! a few awkward options:
//!
//! 1. Give up on type safety until we're done merging - Type safety is nice, and we still need to produce a schema for
//!    Kubernetes to validate against.
//! 2. Give on distinguishing between pre- and post-validation types - Type safety is nice, and it gets error-prone having to memorize
//!    which [`Option::unwrap`]s are completely benign, and which are going to bring down the whole cluster. And, uh, good luck trying
//!    to *change* that in either direction.
//! 3. Write *separate* types for the pre- and post-validation states - That's a lot of tedious code to have to write twice, and that's not
//!    even counting the validation ([parsing]) and inheritance routines! That's not really stuff you want to get wrong!
//!
//! So far, none of those options look particularly great. 3 would probably be the least unworkable path, but...
//! But then again, uh, we have a compiler. What if we could just make it do the hard work?
//!
//! # Okay, but how does it work?
//!
//! The SCS™©®️️️ is split into two subsystems: [`fragment`] and [`merge`].
//!
//! ## Uhhhh, fragments?
//!
//! The [`Fragment`] macro implements option 3 from above for you. You define the final validated type,
//! and it generates a "Fragment mirror type", where all fields are replaced by [`Option`]al counterparts.
//!
//! For example,
//!
//! ```
//! # use stackable_operator::config::fragment::Fragment;
//! #[derive(Fragment)]
//! struct Foo {
//!     bar: String,
//!     baz: u8,
//! }
//! ```
//!
//! generates this:
//!
//! ```
//! struct FooFragment {
//!     bar: Option<String>,
//!     baz: Option<u8>,
//! }
//! ```
//!
//! Additionally, it provides the [`validate`] function, which lets you turn your `FooFragment` back into a `Foo`
//! (while also making sure that the contents actually make sense).
//!
//! Fragments can also be *nested*, as long as the whole hierarchy has fragments. In this case, the fragment of the substruct will be used,
//! instead of wrapping it in an Option. For example, this:
//!
//! ```
//! # use stackable_operator::config::fragment::Fragment;
//! #[derive(Fragment)]
//! struct Foo {
//!     bar: Bar,
//! }
//!
//! #[derive(Fragment)]
//! struct Bar {
//!     baz: String,
//! }
//! ```
//!
//! generates this:
//!
//! ```
//! struct FooFragment {
//!     bar: BarFragment,
//! }
//!
//! struct BarFragment {
//!     baz: Option<String>,
//! }
//! ```
//!
//! rather than wrapping `Bar` as an option, like this:
//!
//! ```
//! struct FooFragment {
//!     bar: Option<Bar>,
//! }
//!
//! struct Bar {
//!     baz: String,
//! }
//! // BarFragment would be irrelevant here
//! ```
//!
//! ### How does it actually know whether to use a subfragment or an [`Option`]?
//!
//! That's (kind of) a trick question! [`Fragment`] actually has no idea about what an [`Option`] even is!
//! It always uses [`FromFragment::Fragment`]. A type can opt into the [`Option`] treatment by implementing
//! [`Atomic`], which is a marker trait for leaf types that cannot be merged any further.
//!
//! ### And what about defaults? That seems like a pretty big oversight.
//!
//! The Fragment system doesn't natively support default values! Instead, this comes "for free" with the merge system (below).
//! One benefit of this is that the same `Fragment` type can support different default values in different contexts
//! (for example: different defaults in different rolegroups).
//!
//! ### Can I customize my `Fragment` types?
//!
//! Attributes can be applied to the generated types using the `#[fragment_attrs]` attribute. For example,
//! `#[fragment_attrs(derive(Default))]` applies `#[derive(Default)]` to the `Fragment` type.
//!
//! ## And what about merging? So far, those fragments seem pretty useless...
//!
//! This is where the [`Merge`] macro (and trait) comes in! It is designed to be applied to the `Fragment` types (see above),
//! and merges their contents field-by-field, deeply (as in: [`merge`] will recurse into substructs, and merge *their* keys in turn).
//!
//! Just like for `Fragment`s, types can opt out of being merged using the [`Atomic`] trait. This is useful both for "primitive" values
//! (like [`String`], the recursion needs to end *somewhere*, after all), and for values that don't really make sense to merge
//! (like a set of search query parameters).
//!
//! # Fine, how do I actually use it, then?
//!
//! For declarations (in CRDs):
//! - Apply `#[derive(Fragment)] #[fragment_attrs(derive(Merge))]` for your product configuration (and any of its nested types).
//!   - DON'T: `#[derive(Fragment, Merge)]`
//! - Pretty much always derive deserialization and defaulting on the `Fragment`, not the validated type:
//!   - DO: `#[fragment_attrs(derive(Serialize, Deserialize, Default, JsonSchema))]`
//!   - DON'T: `#[derive(Fragment, Serialize, Deserialize, Default, JsonSchema)]`
//! - Refer to the `Fragment` type in CRDs, not the validated type.
//! - Implementing [`Atomic`] if something doesn't make sense to merge.
//! - Define the "validated form" of your configuration: only make fields [`Option`]al if [`None`] is actually a legal value.
//!
//! For runtime code:
//! - Validate and merge with [`RoleGroup::validate_config`] for CRDs, otherwise [`merge`] manually and then validate with [`validate`].
//! - Validate as soon as possible, user code should never read the contents of `Fragment`s.
//! - Defaults are just another layer to be [`merge`]d.
//!
//! [parsing]: https://lexi-lambda.github.io/blog/2019/11/05/parse-don-t-validate/
//! [`merge`]: Merge::merge

pub mod fragment;
pub mod merge;

#[cfg(doc)]
use fragment::{Fragment, FromFragment, validate};
#[cfg(doc)]
use merge::{Atomic, Merge};

#[cfg(doc)]
use crate::role_utils::{Role, RoleGroup};
