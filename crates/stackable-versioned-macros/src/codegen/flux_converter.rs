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
                let resource: #src_lower::#struct_ident = serde_json::from_value(object_spec.clone())
                    .map_err(|err| ConversionError::DeserializeObjectSpec{source: err, kind: stringify!(#enum_ident).to_string()})?;

                #(
                    let resource: #version_chain_string::#struct_ident = resource.into();
                )*

                converted.push(
                    serde_json::to_value(resource)
                        .map_err(|err| ConversionError::SerializeObjectSpec{source: err, kind: stringify!(#enum_ident).to_string()})?
                );
            }}
        },
    );

    Some(quote! {
        #[automatically_derived]
        impl #enum_ident {
            pub fn convert(review: kube::core::conversion::ConversionReview) -> kube::core::conversion::ConversionResponse {
                // Intentionally not using `snafu::ResultExt` here to keep the number of dependencies minimal
                use kube::core::conversion::{ConversionRequest, ConversionResponse};
                use kube::core::response::StatusSummary;
                use stackable_versioned::ConversionError;

                let request = match ConversionRequest::from_review(review) {
                    Ok(request) => request,
                    Err(err) => {
                        return ConversionResponse::invalid(
                            kube::client::Status {
                                status: Some(StatusSummary::Failure),
                                code: 400,
                                message: format!("The ConversionReview send did not include any request: {err}"),
                                reason: "ConversionReview request missing".to_string(),
                                details: None,
                            },
                        );
                    }
                };

                let converted = Self::try_convert(&request);

                let conversion_response = ConversionResponse::for_request(request);
                match converted {
                    Ok(converted) => {
                        conversion_response.success(converted)
                    },
                    Err(err) => {
                        let error_message = err.as_human_readable_error_message();

                        conversion_response.failure(
                            kube::client::Status {
                                status: Some(StatusSummary::Failure),
                                code: err.http_return_code(),
                                message: error_message.clone(),
                                reason: error_message,
                                details: None,
                            },
                        )
                    }
                }
            }

            fn try_convert(request: &kube::core::conversion::ConversionRequest) -> Result<Vec<serde_json::Value>, stackable_versioned::ConversionError> {
                use stackable_versioned::ConversionError;

                // FIXME: Check that request.types.{kind,api_version} match the expected values

                let desired_object_version = <Self as std::str::FromStr>::from_str(&request.desired_api_version)
                    .map_err(|err| ConversionError::ParseDesiredResourceVersion{
                        source: err,
                        version: request.desired_api_version.to_string()
                    })?;

                let mut converted: Vec<serde_json::Value> = Vec::with_capacity(request.objects.len());
                for object in &request.objects {
                    let object_spec = object.get("spec").ok_or_else(|| ConversionError::ObjectHasNoSpec{})?;
                    let object_kind = object.get("kind").ok_or_else(|| ConversionError::ObjectHasNoKind{})?;
                    let object_kind = object_kind.as_str().ok_or_else(|| ConversionError::ObjectKindNotString{kind: object_kind.clone()})?;
                    let object_version = object.get("apiVersion").ok_or_else(|| ConversionError::ObjectHasNoApiVersion{})?;
                    let object_version = object_version.as_str().ok_or_else(|| ConversionError::ObjectApiVersionNotString{api_version: object_version.clone()})?;

                    if object_kind != stringify!(#enum_ident) {
                        return Err(ConversionError::WrongObjectKind{expected_kind: stringify!(#enum_ident).to_string(), send_kind: object_kind.to_string()});
                    }

                    let current_object_version = <Self as std::str::FromStr>::from_str(object_version)
                        .map_err(|err| ConversionError::ParseCurrentResourceVersion{
                            source: err,
                            version: object_version.to_string()
                        })?;

                    match (&current_object_version, &desired_object_version) {
                        #(#matches),*
                    }
                }

                Ok(converted)
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
