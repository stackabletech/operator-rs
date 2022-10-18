use k8s_openapi::apimachinery::pkg::{api::resource::Quantity, apis::meta::v1::LabelSelector};
use std::{
    collections::{btree_map, hash_map, BTreeMap, HashMap},
    hash::Hash,
};

pub use stackable_operator_derive::Merge;

/// A type that can be merged with itself
///
/// This is primarily intended to be implemented for configuration values that can come from several sources, for example
/// configuration files with different scopes (role group, role, cluster) where a tighter scope should take precedence.
///
/// Most users will want to implement this for custom types using [the associated derive macro](`derive@Merge`).
///
/// # Example
///
/// ```
/// # use stackable_operator::config::merge::Merge;
///
/// #[derive(Merge, Debug, PartialEq, Eq)]
/// struct Foo {
///     bar: Option<u8>,
///     baz: Option<u8>,
/// }
///
/// let mut config = Foo {
///     bar: Some(0),
///     baz: None,
/// };
/// config.merge(&Foo {
///     bar: Some(1),
///     baz: Some(2),
/// });
/// assert_eq!(config, Foo {
///     bar: Some(0), // Overridden by `bar: Some(0)` above
///     baz: Some(2), // Fallback is used
/// });
/// ```
///
/// # Options
///
/// A field should be [`Option`]al if it is [`Atomic`] (for example: [`u8`]) or an enum (since the discriminant matters in this case).
/// Composite objects (such as regular structs) should generally *not* be optional.
pub trait Merge {
    /// Merge with `defaults`, preferring values from `self` if they are set there
    fn merge(&mut self, defaults: &Self);
}

impl<T: Merge> Merge for Box<T> {
    fn merge(&mut self, defaults: &Self) {
        T::merge(self, defaults)
    }
}
impl<K: Ord + Clone, V: Merge + Clone> Merge for BTreeMap<K, V> {
    fn merge(&mut self, defaults: &Self) {
        for (k, default_v) in defaults {
            match self.entry(k.clone()) {
                btree_map::Entry::Occupied(mut entry) => {
                    entry.get_mut().merge(default_v);
                }
                btree_map::Entry::Vacant(entry) => {
                    entry.insert(default_v.clone());
                }
            }
        }
    }
}
impl<K: Hash + Eq + Clone, V: Merge + Clone> Merge for HashMap<K, V> {
    fn merge(&mut self, defaults: &Self) {
        for (k, default_v) in defaults {
            match self.entry(k.clone()) {
                hash_map::Entry::Occupied(mut entry) => {
                    entry.get_mut().merge(default_v);
                }
                hash_map::Entry::Vacant(entry) => {
                    entry.insert(default_v.clone());
                }
            }
        }
    }
}

/// Moving version of [`Merge::merge`], to produce slightly nicer test output
pub fn merge<T: Merge>(mut overrides: T, defaults: &T) -> T {
    overrides.merge(defaults);
    overrides
}

/// Composable version of [`Merge::merge`] that allows reducing a sequence of `Option<mut& T>`.
///
/// Example:
///
/// ```
/// use stackable_operator::config::merge::{Merge, chainable_merge};
/// #[derive(Clone, Default, Merge, PartialEq)]
/// struct MyConfig {
///     field: Option<i32>,
/// }
///
/// let mut c0 = None;
/// let mut c1 = Some(MyConfig { field: Some(23) });
/// let mut c2 = Some(MyConfig { field: Some(7) });
///
/// let merged = [c0.as_mut(), c1.as_mut(), c2.as_mut()]
///     .into_iter()
///     .flatten()
///     .reduce(|old, new| chainable_merge(new, old));
///
/// assert_eq!(7, merged.unwrap().field.unwrap());
/// ```
pub fn chainable_merge<'a, T: Merge + Clone>(this: &'a mut T, defaults: &T) -> &'a mut T {
    this.merge(defaults);
    this
}

/// A marker trait for types that are merged atomically (as one single value) rather than
/// trying to merge each field individually
pub trait Atomic: Clone {}
impl Atomic for u8 {}
impl Atomic for u16 {}
impl Atomic for u32 {}
impl Atomic for u64 {}
impl Atomic for u128 {}
impl Atomic for usize {}
impl Atomic for i8 {}
impl Atomic for i16 {}
impl Atomic for i32 {}
impl Atomic for i64 {}
impl Atomic for i128 {}
impl Atomic for isize {}
impl Atomic for bool {}
impl Atomic for String {}
impl Atomic for Quantity {}
impl<'a> Atomic for &'a str {}
impl Atomic for LabelSelector {}

impl<T: Atomic> Merge for Option<T> {
    fn merge(&mut self, defaults: &Self) {
        if self.is_none() {
            *self = defaults.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use super::{merge, Merge};

    #[derive(Debug, PartialEq, Eq, Clone)]
    struct Accumulator(u8);
    impl Merge for Accumulator {
        fn merge(&mut self, defaults: &Self) {
            self.0 += defaults.0
        }
    }

    #[test]
    fn merge_derived_struct() {
        #[derive(Merge, PartialEq, Eq, Debug)]
        #[merge(path_overrides(merge = "super"))]
        struct Mergeable {
            one: Option<u8>,
            two: Option<bool>,
        }

        assert_eq!(
            merge(
                Mergeable {
                    one: None,
                    two: None,
                },
                &Mergeable {
                    one: Some(1),
                    two: None,
                }
            ),
            Mergeable {
                one: Some(1),
                two: None,
            }
        );
        assert_eq!(
            merge(
                Mergeable {
                    one: Some(0),
                    two: None,
                },
                &Mergeable {
                    one: Some(1),
                    two: None,
                }
            ),
            Mergeable {
                one: Some(0),
                two: None,
            }
        );
        assert_eq!(
            merge(
                Mergeable {
                    one: Some(0),
                    two: None,
                },
                &Mergeable {
                    one: Some(1),
                    two: Some(false),
                }
            ),
            Mergeable {
                one: Some(0),
                two: Some(false),
            }
        );
    }

    #[test]
    fn merge_nested_derived_struct() {
        #[derive(Merge, PartialEq, Eq, Debug)]
        #[merge(path_overrides(merge = "super"))]
        struct Parent {
            one: Option<u8>,
            child: Child,
        }
        #[derive(Merge, PartialEq, Eq, Debug)]
        #[merge(path_overrides(merge = "super"))]
        struct Child {
            two: Option<u8>,
            three: Option<bool>,
        }

        assert_eq!(
            merge(
                Parent {
                    one: Some(0),
                    child: Child {
                        two: None,
                        three: Some(true),
                    }
                },
                &Parent {
                    one: None,
                    child: Child {
                        two: Some(1),
                        three: Some(false),
                    }
                },
            ),
            Parent {
                one: Some(0),
                child: Child {
                    two: Some(1),
                    three: Some(true)
                },
            }
        );
    }

    #[test]
    fn merge_derived_struct_with_generics() {
        #[derive(Merge, PartialEq, Eq, Debug)]
        #[merge(bound = "B: Merge", path_overrides(merge = "super"))]
        struct Mergeable<'a, B, const C: u8> {
            one: Option<&'a str>,
            two: B,
            three: ParametrizedUnit<C>,
        }
        #[derive(PartialEq, Eq, Debug)]
        struct ParametrizedUnit<const N: u8>;
        impl<const N: u8> Merge for ParametrizedUnit<N> {
            fn merge(&mut self, _defaults: &Self) {}
        }

        assert_eq!(
            merge(
                Mergeable {
                    one: None,
                    two: Some(23),
                    three: ParametrizedUnit::<23>,
                },
                &Mergeable {
                    one: Some("abc"),
                    two: None,
                    three: ParametrizedUnit,
                },
            ),
            Mergeable {
                one: Some("abc"),
                two: Some(23),
                three: ParametrizedUnit,
            }
        );
    }

    #[test]
    fn merge_derived_tuple_struct() {
        #[derive(Merge, PartialEq, Eq, Debug)]
        #[merge(path_overrides(merge = "super"))]
        struct Mergeable(Option<u8>, Option<u16>);

        assert_eq!(
            merge(Mergeable(Some(1), None), &Mergeable(Some(2), Some(3))),
            Mergeable(Some(1), Some(3))
        );
    }

    #[test]
    fn merge_derived_enum() {
        #[derive(Merge, PartialEq, Eq, Debug, Clone)]
        #[merge(path_overrides(merge = "super"))]
        enum Mergeable {
            Foo { one: Option<u8>, two: Option<u16> },
            Bar(Option<u32>),
        }

        assert_eq!(
            merge(
                Some(Mergeable::Foo {
                    one: Some(1),
                    two: None,
                }),
                &Some(Mergeable::Foo {
                    one: Some(2),
                    two: Some(3),
                }),
            ),
            Some(Mergeable::Foo {
                one: Some(1),
                two: Some(3),
            })
        );

        assert_eq!(
            merge(
                Some(Mergeable::Foo {
                    one: Some(1),
                    two: Some(2),
                }),
                &None,
            ),
            Some(Mergeable::Foo {
                one: Some(1),
                two: Some(2),
            })
        );
        assert_eq!(
            merge(
                None,
                &Some(Mergeable::Foo {
                    one: Some(1),
                    two: Some(2),
                }),
            ),
            Some(Mergeable::Foo {
                one: Some(1),
                two: Some(2),
            })
        );

        assert_eq!(
            merge(
                Some(Mergeable::Foo {
                    one: None,
                    two: None,
                }),
                &Some(Mergeable::Bar(None))
            ),
            Some(Mergeable::Foo {
                one: None,
                two: None,
            })
        );

        // This is more of a consequence of how enums are merged, but it's worth calling out explicitly
        // When the enum variant mismatches, *all* default fields are discarded and entirely replaced with the new variant
        assert_eq!(
            merge(
                Some(Mergeable::Foo {
                    one: Some(1),
                    two: None,
                }),
                &merge(
                    Some(Mergeable::Bar(None)),
                    &Some(Mergeable::Foo {
                        one: None,
                        two: Some(2),
                    })
                )
            ),
            Some(Mergeable::Foo {
                one: Some(1),
                two: None,
            })
        );
    }

    #[test]
    fn merge_hash_map() {
        use self::Accumulator as Acc;
        assert_eq!(
            merge(
                HashMap::from([("a", Acc(1)), ("b", Acc(2))]),
                &[("a", Acc(3)), ("c", Acc(5))].into()
            ),
            HashMap::from([("a", Acc(4)), ("b", Acc(2)), ("c", Acc(5))])
        );
    }

    #[test]
    fn merge_btree_map() {
        use self::Accumulator as Acc;
        assert_eq!(
            merge(
                BTreeMap::from([("a", Acc(1)), ("b", Acc(2))]),
                &[("a", Acc(3)), ("c", Acc(5))].into()
            ),
            BTreeMap::from([("a", Acc(4)), ("b", Acc(2)), ("c", Acc(5))])
        );
    }
}
