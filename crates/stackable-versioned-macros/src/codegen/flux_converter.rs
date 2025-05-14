use darling::util::IdentString;
use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn generate_kubernetes_conversion(
    enum_ident: &IdentString,
    struct_ident: &IdentString,
    enum_variant_idents: &[IdentString],
    enum_variant_strings: &[String],
) -> Option<TokenStream> {
    assert_eq!(enum_variant_idents.len(), enum_variant_strings.len());

    let versions = enum_variant_idents
        .iter()
        .zip(enum_variant_strings)
        .collect::<Vec<_>>();
    let conversion_chain = generate_conversion_chain(versions);

    let matches = conversion_chain.into_iter().map(
        |((src, src_lower), (dst, _dst_lower), version_chain)| {
            let version_chain_string = version_chain.iter()
                .map(|(_,v)| v.parse::<TokenStream>()
                    .expect("The versions always needs to be a valid TokenStream"));

            // TODO: Is there a bit more clever way how we can get this?
            let src_lower = src_lower.parse::<TokenStream>().expect("The versions always needs to be a valid TokenStream");

            quote! { (Self::#src, Self::#dst) => {
                let resource: #src_lower::#struct_ident = serde_json::from_value(object_spec)
                    .expect(&format!("Failed to deserialize {}", stringify!(#enum_ident)));

                #(
                    let resource: #version_chain_string::#struct_ident = resource.into();
                )*

                converted.push(
                    serde_json::to_value(resource).expect(&format!("Failed to serialize {}", stringify!(#enum_ident)))
                );
            }}
        },
    );

    Some(quote! {
        #[automatically_derived]
        impl #enum_ident {
            pub fn convert(review: kube::core::conversion::ConversionReview) -> kube::core::conversion::ConversionResponse {
                let request = kube::core::conversion::ConversionRequest::from_review(review)
                    .unwrap();
                let desired_api_version = <Self as std::str::FromStr>::from_str(&request.desired_api_version)
                    .expect(&format!("invalid desired version for {} resource", stringify!(#enum_ident)));

                let mut converted: Vec<serde_json::Value> = Vec::with_capacity(request.objects.len());
                for object in &request.objects {
                    let object_spec = object
                        .get("spec")
                        .expect("The passed object had no spec")
                        .clone();
                    let kind = object
                        .get("kind")
                        .expect("The objected asked to convert has no kind");
                    let api_version = object
                        .get("apiVersion")
                        .expect("The objected asked to convert has no apiVersion")
                        .as_str()
                        .expect("The apiVersion of the objected asked to convert wasn't a String");

                    assert_eq!(kind, stringify!(#enum_ident));

                    let current_api_version = <Self as std::str::FromStr>::from_str(api_version)
                        .expect(&format!("invalid current version for {} resource", stringify!(#enum_ident)));

                    match (&current_api_version, &desired_api_version) {
                        #(#matches),*
                    }
                }

                let response = kube::core::conversion::ConversionResponse::for_request(request);
                response.success(converted)
            }
        }
    })
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
            let chain = if i == j {
                vec![]
            } else if i < j {
                versions[i + 1..=j].to_vec()
            } else {
                versions[j..i].iter().rev().cloned().collect()
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

        assert_eq!(conversion_chain, vec![
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
            ("v2", "v1alpha1", vec![
                "v1", "v1beta1", "v1alpha2", "v1alpha1"
            ]),
            ("v2", "v1alpha2", vec!["v1", "v1beta1", "v1alpha2"]),
            ("v2", "v1beta1", vec!["v1", "v1beta1"]),
            ("v2", "v1", vec!["v1"]),
            ("v2", "v2", vec![])
        ]);
    }
}
