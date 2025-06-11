use std::cmp::Ordering;

use darling::util::IdentString;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::attrs::container::k8s::KubernetesArguments;

pub(crate) fn generate_kubernetes_conversion_tests(
    enum_ident: &IdentString,
    struct_ident: &IdentString,
    enum_variant_strings: &[String],
    kubernetes_options: &KubernetesArguments,
) -> TokenStream {
    // Get the crate paths
    let versioned_path = &*kubernetes_options.crates.versioned;

    let k8s_group = &kubernetes_options.group;

    let earliest_version = enum_variant_strings.first().expect(&format!(
        "There must be a earliest version in the list of versions for {enum_ident}"
    ));
    let latest_version = enum_variant_strings.last().expect(&format!(
        "There must be a latest version in the list of versions for {enum_ident}"
    ));
    let earliest_api_version = format!("{k8s_group}/{earliest_version}");
    let latest_api_version = format!("{k8s_group}/{latest_version}");

    let earliest_version_ident = format_ident!("{earliest_version}");
    let latest_version_ident = format_ident!("{latest_version}");
    let test_function_down_up = format_ident!("{enum_ident}_roundtrip_down_up");
    let test_function_up_down = format_ident!("{enum_ident}_roundtrip_up_down");

    quote! {
        #[cfg(test)]
        #[test]
        fn #test_function_down_up() {
            #versioned_path::flux_converter::test_utils::test_roundtrip::<
                #latest_version_ident::#struct_ident,
            >(
                stringify!(#enum_ident),
                #latest_api_version,
                #earliest_api_version,
                #enum_ident::convert,
            );
        }

        #[cfg(test)]
        #[test]
        fn #test_function_up_down() {
            #versioned_path::flux_converter::test_utils::test_roundtrip::<
                #earliest_version_ident::#struct_ident,
            >(
                stringify!(#enum_ident),
                #earliest_api_version,
                #latest_api_version,
                #enum_ident::convert,
            );
        }
    }
}

pub fn generate_conversion_chain<Version: Clone>(
    versions: Vec<Version>,
) -> Vec<(Version, Version, Vec<Version>)> {
    let mut result = Vec::with_capacity(versions.len().pow(2));
    let n = versions.len();

    for i in 0..n {
        for j in 0..n {
            let source = versions[i].clone();
            let destination = versions[j].clone();

            let chain = match i.cmp(&j) {
                Ordering::Equal => vec![],
                Ordering::Less => versions[i + 1..=j].to_vec(),
                Ordering::Greater => versions[j..i].iter().rev().cloned().collect(),
            };

            result.push((source, destination, chain));
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::generate_conversion_chain;

    #[test]
    fn test_generate_conversion_chains() {
        let versions = vec!["v1alpha1", "v1alpha2", "v1beta1", "v1", "v2"];
        let conversion_chain = generate_conversion_chain(versions);

        assert_eq!(
            conversion_chain,
            vec![
                ("v1alpha1", "v1alpha1", vec![]),
                ("v1alpha1", "v1alpha2", vec!["v1alpha2"]),
                ("v1alpha1", "v1beta1", vec!["v1alpha2", "v1beta1"]),
                ("v1alpha1", "v1", vec!["v1alpha2", "v1beta1", "v1"]),
                ("v1alpha1", "v2", vec!["v1alpha2", "v1beta1", "v1", "v2"]),
                ("v1alpha2", "v1alpha1", vec!["v1alpha1"]),
                ("v1alpha2", "v1alpha2", vec![]),
                ("v1alpha2", "v1beta1", vec!["v1beta1"]),
                ("v1alpha2", "v1", vec!["v1beta1", "v1"]),
                ("v1alpha2", "v2", vec!["v1beta1", "v1", "v2"]),
                ("v1beta1", "v1alpha1", vec!["v1alpha2", "v1alpha1"]),
                ("v1beta1", "v1alpha2", vec!["v1alpha2"]),
                ("v1beta1", "v1beta1", vec![]),
                ("v1beta1", "v1", vec!["v1"]),
                ("v1beta1", "v2", vec!["v1", "v2"]),
                ("v1", "v1alpha1", vec!["v1beta1", "v1alpha2", "v1alpha1"]),
                ("v1", "v1alpha2", vec!["v1beta1", "v1alpha2"]),
                ("v1", "v1beta1", vec!["v1beta1"]),
                ("v1", "v1", vec![]),
                ("v1", "v2", vec!["v2"]),
                (
                    "v2",
                    "v1alpha1",
                    vec!["v1", "v1beta1", "v1alpha2", "v1alpha1"]
                ),
                ("v2", "v1alpha2", vec!["v1", "v1beta1", "v1alpha2"]),
                ("v2", "v1beta1", vec!["v1", "v1beta1"]),
                ("v2", "v1", vec!["v1"]),
                ("v2", "v2", vec![])
            ]
        );
    }
}
