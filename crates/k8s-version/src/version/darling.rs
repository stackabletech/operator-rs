use std::str::FromStr;

use darling::FromMeta;

use crate::Version;

impl FromMeta for Version {
    fn from_string(value: &str) -> darling::Result<Self> {
        Self::from_str(value).map_err(darling::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use rstest::rstest;

    use super::*;
    use crate::Level;

    fn parse_meta(tokens: proc_macro2::TokenStream) -> ::std::result::Result<syn::Meta, String> {
        let attribute: syn::Attribute = syn::parse_quote!(#[#tokens]);
        Ok(attribute.meta)
    }

    #[cfg(feature = "darling")]
    #[rstest]
    #[case(quote!(ignore = "v1alpha12"), Version { major: 1, level: Some(Level::Alpha(12)) })]
    #[case(quote!(ignore = "v1alpha1"), Version { major: 1, level: Some(Level::Alpha(1)) })]
    #[case(quote!(ignore = "v1beta1"), Version { major: 1, level: Some(Level::Beta(1)) })]
    #[case(quote!(ignore = "v1"), Version { major: 1, level: None })]
    fn from_meta(#[case] input: proc_macro2::TokenStream, #[case] expected: Version) {
        let meta = parse_meta(input).expect("valid attribute tokens");
        let version = Version::from_meta(&meta).expect("version must parse from attribute");
        assert_eq!(version, expected);
    }
}
