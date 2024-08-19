use std::{collections::BTreeMap, ops::Bound};

pub(crate) trait Neighbors<K, V>
where
    K: Ord + Eq,
{
    fn get_neighbors(&self, key: &K) -> (Option<&V>, Option<&V>);

    fn lo_bound(&self, bound: Bound<&K>) -> Option<(&K, &V)>;
    fn up_bound(&self, bound: Bound<&K>) -> Option<(&K, &V)>;
}

impl<K, V> Neighbors<K, V> for BTreeMap<K, V>
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

impl<K, V> BTreeMapExt<K, V> for BTreeMap<K, V>
where
    K: Ord,
{
    const MESSAGE: &'static str = "internal error: chain must contain version";

    fn get_expect(&self, key: &K) -> &V {
        self.get(key).expect(Self::MESSAGE)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(0, (None, Some(&"test1")))]
    #[case(1, (None, Some(&"test3")))]
    #[case(2, (Some(&"test1"), Some(&"test3")))]
    #[case(3, (Some(&"test1"), None))]
    #[case(4, (Some(&"test3"), None))]
    fn test(#[case] key: i32, #[case] expected: (Option<&&str>, Option<&&str>)) {
        let map = BTreeMap::from([(1, "test1"), (3, "test3")]);
        let neigbors = map.get_neighbors(&key);

        assert_eq!(neigbors, expected);
    }
}
