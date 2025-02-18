use std::{collections::BTreeMap, ops::Bound};

use k8s_version::Version;
use syn::Type;

use crate::codegen::{ItemStatus, VersionDefinition};

pub(crate) trait Neighbors<K, V>
where
    K: Ord + Eq,
{
    /// Returns the values of keys which are neighbors of `key`.
    ///
    /// Given a map which contains the following keys: 1, 3, 5. Calling this
    /// function with these keys, results in the following return values:
    ///
    /// - Key **0**: `(None, Some(1))`
    /// - Key **2**: `(Some(1), Some(3))`
    /// - Key **4**: `(Some(3), Some(5))`
    /// - Key **6**: `(Some(5), None)`
    fn get_neighbors(&self, key: &K) -> (Option<&V>, Option<&V>);

    /// Returns whether the function `f` returns true if applied to the value
    /// identified by `key`.
    fn value_is<F>(&self, key: &K, f: F) -> bool
    where
        F: Fn(&V) -> bool;

    fn lo_bound(&self, bound: Bound<&K>) -> Option<(&K, &V)>;
    fn up_bound(&self, bound: Bound<&K>) -> Option<(&K, &V)>;
}

impl<K, V> Neighbors<K, V> for BTreeMap<K, V>
where
    K: Ord + Eq,
{
    fn get_neighbors(&self, key: &K) -> (Option<&V>, Option<&V>) {
        // NOTE (@Techassi): These functions might get added to the standard
        // library at some point. If that's the case, we can use the ones
        // provided by the standard lib.
        // See: https://github.com/rust-lang/rust/issues/107540
        match (
            self.lo_bound(Bound::Excluded(key)),
            self.up_bound(Bound::Excluded(key)),
        ) {
            (Some((k, v)), None) => {
                if key > k {
                    (Some(v), None)
                } else {
                    (self.lo_bound(Bound::Excluded(k)).map(|(_, v)| v), None)
                }
            }
            (None, Some((k, v))) => {
                if key < k {
                    (None, Some(v))
                } else {
                    (None, self.up_bound(Bound::Excluded(k)).map(|(_, v)| v))
                }
            }
            (Some((_, lo)), Some((_, up))) => (Some(lo), Some(up)),
            (None, None) => unreachable!(),
        }
    }

    fn value_is<F>(&self, key: &K, f: F) -> bool
    where
        F: Fn(&V) -> bool,
    {
        self.get(key).map_or(false, f)
    }

    fn lo_bound(&self, bound: Bound<&K>) -> Option<(&K, &V)> {
        self.range((Bound::Unbounded, bound)).next_back()
    }

    fn up_bound(&self, bound: Bound<&K>) -> Option<(&K, &V)> {
        self.range((bound, Bound::Unbounded)).next()
    }
}

pub(crate) trait BTreeMapExt<K, V>
where
    K: Ord,
{
    const MESSAGE: &'static str;

    fn get_expect(&self, key: &K) -> &V;
}

impl<V> BTreeMapExt<Version, V> for BTreeMap<Version, V> {
    const MESSAGE: &'static str = "internal error: chain must contain version";

    fn get_expect(&self, key: &Version) -> &V {
        self.get(key).expect(Self::MESSAGE)
    }
}

pub(crate) trait ChangesetExt {
    fn insert_container_versions(&mut self, versions: &[VersionDefinition], ty: &Type);
}

impl ChangesetExt for BTreeMap<Version, ItemStatus> {
    fn insert_container_versions(&mut self, versions: &[VersionDefinition], ty: &Type) {
        for version in versions {
            if self.contains_key(&version.inner) {
                continue;
            }

            match self.get_neighbors(&version.inner) {
                (None, Some(status)) => match status {
                    ItemStatus::Addition { .. } => {
                        self.insert(version.inner, ItemStatus::NotPresent)
                    }
                    ItemStatus::Change {
                        from_ident,
                        from_type,
                        ..
                    } => self.insert(
                        version.inner,
                        ItemStatus::NoChange {
                            previously_deprecated: false,
                            ident: from_ident.clone(),
                            ty: from_type.clone(),
                        },
                    ),
                    ItemStatus::Deprecation { previous_ident, .. } => self.insert(
                        version.inner,
                        ItemStatus::NoChange {
                            previously_deprecated: false,
                            ident: previous_ident.clone(),
                            ty: ty.clone(),
                        },
                    ),
                    ItemStatus::NoChange {
                        previously_deprecated,
                        ident,
                        ty,
                    } => self.insert(
                        version.inner,
                        ItemStatus::NoChange {
                            previously_deprecated: *previously_deprecated,
                            ident: ident.clone(),
                            ty: ty.clone(),
                        },
                    ),
                    ItemStatus::NotPresent => unreachable!(),
                },
                (Some(status), None) => {
                    let (ident, ty, previously_deprecated) = match status {
                        ItemStatus::Addition { ident, ty, .. } => (ident, ty, false),
                        ItemStatus::Change {
                            to_ident, to_type, ..
                        } => (to_ident, to_type, false),
                        ItemStatus::Deprecation { ident, .. } => (ident, ty, true),
                        ItemStatus::NoChange {
                            previously_deprecated,
                            ident,
                            ty,
                            ..
                        } => (ident, ty, *previously_deprecated),
                        ItemStatus::NotPresent => unreachable!(),
                    };

                    self.insert(
                        version.inner,
                        ItemStatus::NoChange {
                            previously_deprecated,
                            ident: ident.clone(),
                            ty: ty.clone(),
                        },
                    )
                }
                (Some(status), Some(_)) => {
                    let (ident, ty, previously_deprecated) = match status {
                        ItemStatus::Addition { ident, ty, .. } => (ident, ty, false),
                        ItemStatus::Change {
                            to_ident, to_type, ..
                        } => (to_ident, to_type, false),
                        ItemStatus::NoChange {
                            previously_deprecated,
                            ident,
                            ty,
                            ..
                        } => (ident, ty, *previously_deprecated),
                        // TODO (@NickLarsenNZ): Explain why it is unreachable, as it can be reached during testing.
                        // To reproduce, use an invalid version, eg: #[versioned(deprecated(since = "v99"))]
                        _ => unreachable!(),
                    };

                    self.insert(
                        version.inner,
                        ItemStatus::NoChange {
                            previously_deprecated,
                            ident: ident.clone(),
                            ty: ty.clone(),
                        },
                    )
                }
                _ => unreachable!(),
            };
        }
    }
}

#[cfg(test)]
mod test {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(0, (None, Some(&"test1")))]
    #[case(1, (None, Some(&"test3")))]
    #[case(2, (Some(&"test1"), Some(&"test3")))]
    #[case(3, (Some(&"test1"), None))]
    #[case(4, (Some(&"test3"), None))]
    fn neighbors(#[case] key: i32, #[case] expected: (Option<&&str>, Option<&&str>)) {
        let map = BTreeMap::from([(1, "test1"), (3, "test3")]);
        let neigbors = map.get_neighbors(&key);

        assert_eq!(neigbors, expected);
    }
}
