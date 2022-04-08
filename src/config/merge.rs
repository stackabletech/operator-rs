pub use stackable_operator_derive::Merge;

/// A type that can be merged with itself
///
/// This is primarily intended to be implemented for configuration values that can come from several sources, for example
/// configuration files with different scopes (role group, role, cluster) where a tighter scope should take precedence.
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
pub trait Merge {
    /// Merge with `defaults`, preferring values from `self` if they are set there
    fn merge(&mut self, defaults: &Self);
}

/// A marker trait for types that are merged atomically and have no subfields
trait Atomic: Clone {}
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

impl<T: Atomic> Merge for Option<T> {
    fn merge(&mut self, defaults: &Self) {
        if self.is_none() {
            *self = defaults.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Merge;

    /// Moving version of [`Merge::merge`], to produce slightly nicer test output
    fn merge<T: Merge>(mut overrides: T, defaults: &T) -> T {
        overrides.merge(defaults);
        overrides
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
}
