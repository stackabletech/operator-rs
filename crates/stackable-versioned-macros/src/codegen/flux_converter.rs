use std::cmp::Ordering;

use darling::util::IdentString;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::container::KubernetesOptions;

pub(crate) fn generate_kubernetes_conversion(
    enum_ident: &IdentString,
    struct_ident: &IdentString,
    enum_variant_idents: &[IdentString],
    enum_variant_strings: &[String],
    kubernetes_options: &KubernetesOptions,
) -> Option<TokenStream> {
    assert_eq!(enum_variant_idents.len(), enum_variant_strings.len());

    // Get the crate paths
    let kube_core_path = &*kubernetes_options.crates.kube_core;
    let kube_client_path = &*kubernetes_options.crates.kube_client;
    let versioned_path = &*kubernetes_options.crates.versioned;

    let versions = enum_variant_idents
        .iter()
        .zip(enum_variant_strings)
        .collect::<Vec<_>>();
    let conversion_chain = generate_conversion_chain(versions);

    let matches = conversion_chain.into_iter().map(
        |((src, src_lower), (dst, dst_lower), version_chain)| {
            let steps = version_chain.len();
            let version_chain_string = version_chain.iter()
                .map(|(_,v)| v.parse::<TokenStream>()
                    .expect("The versions always needs to be a valid TokenStream"));

            // TODO: Is there a bit more clever way how we can get this?
            let src_lower = src_lower.parse::<TokenStream>().expect("The versions always needs to be a valid TokenStream");

            quote! { (Self::#src, Self::#dst) => {
                let resource_spec: #src_lower::#struct_ident = serde_json::from_value(object_spec.clone())
                    .map_err(|err| ConversionError::DeserializeObjectSpec{source: err, kind: stringify!(#enum_ident).to_string()})?;

                #(
                    let resource_spec: #version_chain_string::#struct_ident = resource_spec.into();
                )*

                tracing::trace!(
                    from = stringify!(#src_lower),
                    to = stringify!(#dst_lower),
                    conversion.steps = #steps,
                    "Successfully converted {type} object",
                    type = stringify!(#enum_ident),
                );

                let mut object = object.clone();
                *object.get_mut("spec").ok_or_else(|| ConversionError::ObjectHasNoSpec{})? = serde_json::to_value(resource_spec)
                        .map_err(|err| ConversionError::SerializeObjectSpec{source: err, kind: stringify!(#enum_ident).to_string()})?;
                *object.get_mut("apiVersion").ok_or_else(|| ConversionError::ObjectHasNoApiVersion{})?
                    = serde_json::Value::String(request.desired_api_version.clone());
                converted.push(object);
            }}
        },
    );

    Some(quote! {
        #[automatically_derived]
        impl #enum_ident {
            #[tracing::instrument(
                skip_all,
                fields(
                    conversion.kind = review.types.kind,
                    conversion.api_version = review.types.api_version,
                )
            )]
            pub fn convert(review: #kube_core_path::conversion::ConversionReview) -> #kube_core_path::conversion::ConversionReview {
                // Intentionally not using `snafu::ResultExt` here to keep the number of dependencies minimal
                use #kube_core_path::conversion::{ConversionRequest, ConversionResponse};
                use #kube_core_path::response::StatusSummary;
                use #versioned_path::flux_converter::ConversionError;

                let request = match ConversionRequest::from_review(review) {
                    Ok(request) => request,
                    Err(err) => {
                        tracing::warn!(
                            ?err,
                            "Invalid ConversionReview send by Kubernetes apiserver. It probably did not include a request"
                        );

                        return ConversionResponse::invalid(
                            #kube_client_path::Status {
                                status: Some(StatusSummary::Failure),
                                code: 400,
                                message: format!("The ConversionReview send did not include any request: {err}"),
                                reason: "ConversionReview request missing".to_string(),
                                details: None,
                            },
                        ).into_review();
                    }
                };

                let converted = Self::try_convert(&request);

                let conversion_response = ConversionResponse::for_request(request);
                match converted {
                    Ok(converted) => {
                        tracing::debug!(
                            "Successfully converted {num} objects of type {type}",
                            num = converted.len(),
                            type = stringify!(#enum_ident),
                        );

                        conversion_response.success(converted).into_review()
                    },
                    Err(err) => {
                        let error_message = err.as_human_readable_error_message();

                        conversion_response.failure(
                            #kube_client_path::Status {
                                status: Some(StatusSummary::Failure),
                                code: err.http_return_code(),
                                message: error_message.clone(),
                                reason: error_message,
                                details: None,
                            },
                        ).into_review()
                    }
                }
            }

            #[tracing::instrument(
                skip_all,
                err
            )]
            fn try_convert(request: &#kube_core_path::conversion::ConversionRequest) -> Result<
                Vec<serde_json::Value>,
                #versioned_path::flux_converter::ConversionError
            > {
                use #versioned_path::flux_converter::ConversionError;

                // FIXME: Check that request.types.{kind,api_version} match the expected values

                let desired_object_version = Self::from_api_version(&request.desired_api_version)
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

                    let current_object_version = Self::from_api_version(object_version)
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

pub(crate) fn generate_kubernetes_conversion_tests(
    enum_ident: &IdentString,
    struct_ident: &IdentString,
    enum_variant_strings: &[String],
    kubernetes_options: &KubernetesOptions,
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
