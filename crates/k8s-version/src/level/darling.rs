use std::str::FromStr;

use darling::FromMeta;

use crate::Level;

impl FromMeta for Level {
    fn from_string(value: &str) -> darling::Result<Self> {
        Self::from_str(value).map_err(darling::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use rstest::rstest;

    use super::*;

    fn parse_meta(tokens: proc_macro2::TokenStream) -> ::std::result::Result<syn::Meta, String> {
        let attribute: syn::Attribute = syn::parse_quote!(#[#tokens]);
        Ok(attribute.meta)
    }

    #[rstest]
    #[case(quote!(ignore = "alpha12"), Level::Alpha(12))]
    #[case(quote!(ignore = "alpha1"), Level::Alpha(1))]
    #[case(quote!(ignore = "beta1"), Level::Beta(1))]
    fn from_meta(#[case] input: proc_macro2::TokenStream, #[case] expected: Level) {
        let meta = parse_meta(input).expect("valid attribute tokens");
        let version = Level::from_meta(&meta).expect("level must parse from attribute");
        assert_eq!(version, expected);
    }
}
