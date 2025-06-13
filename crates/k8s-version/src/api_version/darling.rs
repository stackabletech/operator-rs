use std::str::FromStr;

use darling::FromMeta;

use crate::ApiVersion;

impl FromMeta for ApiVersion {
    fn from_string(value: &str) -> darling::Result<Self> {
        Self::from_str(value).map_err(darling::Error::custom)
    }
}

#[cfg(test)]
mod test {
    use quote::quote;
    use rstest::rstest;

    use super::*;
    use crate::{Level, Version};

    fn parse_meta(tokens: proc_macro2::TokenStream) -> ::std::result::Result<syn::Meta, String> {
        let attribute: syn::Attribute = syn::parse_quote!(#[#tokens]);
        Ok(attribute.meta)
    }

    #[rstest]
    #[case(quote!(ignore = "extensions/v1beta1"), ApiVersion { group: Some("extensions".parse().unwrap()), version: Version { major: 1, level: Some(Level::Beta(1)) } })]
    #[case(quote!(ignore = "v1beta1"), ApiVersion { group: None, version: Version { major: 1, level: Some(Level::Beta(1)) } })]
    #[case(quote!(ignore = "v1"), ApiVersion { group: None, version: Version { major: 1, level: None } })]
    fn from_meta(#[case] input: proc_macro2::TokenStream, #[case] expected: ApiVersion) {
        let meta = parse_meta(input).expect("valid attribute tokens");
        let api_version = ApiVersion::from_meta(&meta).expect("version must parse from attribute");
        assert_eq!(api_version, expected);
    }
}
