use std::{collections::BTreeMap, ops::Bound};

use darling::FromDeriveInput;
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{spanned::Spanned, Data, DeriveInput, Error, Result};

use crate::{
    attrs::container::ContainerAttributes,
    gen::{venum::VersionedEnum, version::ContainerVersion, vstruct::VersionedStruct},
};

pub(crate) mod field;
pub(crate) mod venum;
pub(crate) mod version;
pub(crate) mod vstruct;

// NOTE (@Techassi): This derive macro cannot handle multiple structs / enums
// to be versioned within the same file. This is because we cannot declare
// modules more than once (They will not be merged, like impl blocks for
// example). This leads to collisions if there are multiple structs / enums
// which declare the same version. This could maybe be solved by using an
// attribute macro applied to a module with all struct / enums declared in said
// module. This would allow us to generate all versioned structs and enums in
// a single sweep and put them into the appropriate module.

// TODO (@Techassi): Think about how we can handle nested structs / enums which
// are also versioned.

pub(crate) fn expand(input: DeriveInput) -> Result<TokenStream> {
    // Extract container attributes
    let attributes = ContainerAttributes::from_derive_input(&input)?;

    // Validate container shape and generate code
    let expanded = match input.data {
        Data::Struct(data) => {
            VersionedStruct::new(input.ident, data, attributes)?.to_token_stream()
        }
        Data::Enum(data) => VersionedEnum::new(input.ident, data, attributes)?.to_token_stream(),
        Data::Union(_) => {
            return Err(Error::new(
                input.span(),
                "derive macro `Versioned` only supports structs and enums",
            ))
        }
    };

    Ok(expanded)
}

pub(crate) trait ToTokensExt {
    fn to_tokens_for_version(&self, version: &ContainerVersion) -> Option<TokenStream>;
}

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
