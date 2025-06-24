use std::{borrow::Cow, cmp::Ordering, ops::Not as _};

use darling::util::IdentString;
use indoc::formatdoc;
use itertools::Itertools as _;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Visibility, parse_quote};

use crate::{
    attrs::container::k8s::KubernetesArguments,
    codegen::{KubernetesTokens, VersionDefinition, container::r#struct::Struct},
    utils::{doc_comments::DocComments, path_to_string},
};

const CONVERTED_OBJECT_COUNT_ATTRIBUTE: &str = "k8s.crd.conversion.converted_object_count";
const DESIRED_API_VERSION_ATTRIBUTE: &str = "k8s.crd.conversion.desired_api_version";
const API_VERSION_ATTRIBUTE: &str = "k8s.crd.conversion.api_version";
const STEPS_ATTRIBUTE: &str = "k8s.crd.conversion.steps";
const KIND_ATTRIBUTE: &str = "k8s.crd.conversion.kind";

impl Struct {
    pub fn generate_kube_attribute(&self, version: &VersionDefinition) -> Option<TokenStream> {
        let kubernetes_arguments = self.common.options.kubernetes_arguments.as_ref()?;

        // Required arguments
        let group = &kubernetes_arguments.group;
        let version = version.inner.to_string();
        let kind = kubernetes_arguments
            .kind
            .as_ref()
            .map_or(self.common.idents.kubernetes.to_string(), |kind| {
                kind.clone()
            });

        // Optional arguments
        let singular = kubernetes_arguments
            .singular
            .as_ref()
            .map(|s| quote! { , singular = #s });

        let plural = kubernetes_arguments
            .plural
            .as_ref()
            .map(|p| quote! { , plural = #p });

        let namespaced = kubernetes_arguments
            .namespaced
            .is_present()
            .then_some(quote! { , namespaced });

        let crates = &kubernetes_arguments.crates;

        let status = match (
            kubernetes_arguments
                .options
                .experimental_conversion_tracking
                .is_present(),
            &kubernetes_arguments.status,
        ) {
            (true, _) => {
                let status_ident = &self.common.idents.kubernetes_status;
                Some(quote! { , status = #status_ident })
            }
            (_, Some(status_ident)) => Some(quote! { , status = #status_ident }),
            (_, _) => None,
        };

        let shortnames: TokenStream = kubernetes_arguments
            .shortnames
            .iter()
            .map(|s| quote! { , shortname = #s })
            .collect();

        Some(quote! {
            // The end-developer needs to derive CustomResource and JsonSchema.
            // This is because we don't know if they want to use a re-exported or renamed import.
            #[kube(
                // These must be comma separated (except the last) as they always exist:
                group = #group, version = #version, kind = #kind
                // These fields are optional, and therefore the token stream must prefix each with a comma:
                #singular #plural #namespaced #crates #status #shortnames
            )]
        })
    }

    pub fn generate_kubernetes_version_items(
        &self,
        version: &VersionDefinition,
    ) -> Option<(TokenStream, IdentString, TokenStream, String)> {
        let kubernetes_arguments = self.common.options.kubernetes_arguments.as_ref()?;

        let module_ident = &version.idents.module;
        let struct_ident = &self.common.idents.kubernetes;

        let variant_data = quote! { #module_ident::#struct_ident };

        let crd_fn = self.generate_kubernetes_crd_fn(version, kubernetes_arguments);
        let variant_ident = version.idents.variant.clone();
        let variant_string = version.inner.to_string();

        Some((crd_fn, variant_ident, variant_data, variant_string))
    }

    fn generate_kubernetes_crd_fn(
        &self,
        version: &VersionDefinition,
        kubernetes_arguments: &KubernetesArguments,
    ) -> TokenStream {
        let kube_core_path = &*kubernetes_arguments.crates.kube_core;
        let struct_ident = &self.common.idents.kubernetes;
        let module_ident = &version.idents.module;

        quote! {
            <#module_ident::#struct_ident as #kube_core_path::CustomResourceExt>::crd()
        }
    }

    pub fn generate_kubernetes_code(
        &self,
        versions: &[VersionDefinition],
        tokens: &KubernetesTokens,
        vis: &Visibility,
        is_nested: bool,
    ) -> Option<TokenStream> {
        let kubernetes_arguments = self.common.options.kubernetes_arguments.as_ref()?;

        // Get various idents needed for code generation
        let variant_data_ident = &self.common.idents.kubernetes_parameter;
        let version_enum_ident = &self.common.idents.kubernetes_version;
        let enum_ident = &self.common.idents.kubernetes;
        let enum_ident_string = enum_ident.to_string();

        // Only add the #[automatically_derived] attribute if this impl is used outside of a
        // module (in standalone mode).
        let automatically_derived = is_nested.not().then(|| quote! {#[automatically_derived]});

        // Get the crate paths
        let k8s_openapi_path = &*kubernetes_arguments.crates.k8s_openapi;
        let serde_json_path = &*kubernetes_arguments.crates.serde_json;
        let versioned_path = &*kubernetes_arguments.crates.versioned;
        let kube_core_path = &*kubernetes_arguments.crates.kube_core;

        // Get the per-version items to be able to iterate over them via quote
        let variant_strings = &tokens.variant_strings;
        let variant_idents = &tokens.variant_idents;
        let variant_data = &tokens.variant_data;
        let crd_fns = &tokens.crd_fns;

        let api_versions = variant_strings
            .iter()
            .map(|version| format!("{group}/{version}", group = &kubernetes_arguments.group));

        // Generate additional Kubernetes code, this is split out to reduce the complexity in this
        // function.
        let status_struct = self.generate_kubernetes_status_struct(kubernetes_arguments, is_nested);
        let version_enum =
            self.generate_kubernetes_version_enum(kubernetes_arguments, tokens, vis, is_nested);
        let convert_method = self.generate_kubernetes_conversion(versions);

        let parse_object_error = quote! { #versioned_path::ParseObjectError };

        Some(quote! {
            #automatically_derived
            #vis enum #enum_ident {
                #(#variant_idents(#variant_data)),*
            }

            #automatically_derived
            impl #enum_ident {
                /// Generates a merged CRD containing all versions and marking `stored_apiversion` as stored.
                pub fn merged_crd(
                    stored_apiversion: #version_enum_ident
                ) -> ::std::result::Result<
                    #k8s_openapi_path::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
                    #kube_core_path::crd::MergeError>
                {
                    #kube_core_path::crd::merge_crds(vec![#(#crd_fns),*], stored_apiversion.as_version_str())
                }

                #convert_method

                fn try_from_json_object(object_value: #serde_json_path::Value) -> ::std::result::Result<Self, #parse_object_error> {
                    let object_kind = object_value
                        .get("kind")
                        .ok_or_else(|| #parse_object_error::FieldMissing{ field: "kind".to_owned() })?
                        .as_str()
                        .ok_or_else(|| #parse_object_error::FieldNotStr{ field: "kind".to_owned() })?;

                    // Note(@sbernauer): The kind must be checked here, because it is
                    // possible for the wrong object to be deserialized.
                    // Checking here stops us assuming the kind is correct and
                    // accidentally updating upgrade/downgrade information in the
                    // status in a later step.
                    if object_kind != #enum_ident_string {
                        return Err(#parse_object_error::UnexpectedKind{
                            kind: object_kind.to_owned(),
                            expected: #enum_ident_string.to_owned(),
                        });
                    }

                    let api_version = object_value
                        .get("apiVersion")
                        .ok_or_else(|| #parse_object_error::FieldMissing{ field: "apiVersion".to_owned() })?
                        .as_str()
                        .ok_or_else(|| #parse_object_error::FieldNotStr{ field: "apiVersion".to_owned() })?;

                    let object = match api_version {
                        #(#api_versions => {
                            let object = #serde_json_path::from_value(object_value)
                                .map_err(|source| #parse_object_error::Deserialize { source })?;

                            Self::#variant_idents(object)
                        },)*
                        unknown_api_version => return ::std::result::Result::Err(#parse_object_error::UnknownApiVersion {
                            api_version: unknown_api_version.to_owned()
                        }),
                    };

                    ::std::result::Result::Ok(object)
                }

                fn into_json_value(self) -> ::std::result::Result<#serde_json_path::Value, #serde_json_path::Error> {
                    match self {
                        #(Self::#variant_idents(#variant_data_ident) => Ok(#serde_json_path::to_value(#variant_data_ident)?),)*
                    }
                }
            }

            #version_enum
            #status_struct
        })
    }

    ////////////////////
    // Merge CRD Code //
    ////////////////////

    fn generate_kubernetes_version_enum(
        &self,
        kubernetes_arguments: &KubernetesArguments,
        tokens: &KubernetesTokens,
        vis: &Visibility,
        is_nested: bool,
    ) -> TokenStream {
        let enum_ident = &self.common.idents.kubernetes_version;

        // Only add the #[automatically_derived] attribute if this impl is used outside of a
        // module (in standalone mode).
        let automatically_derived = is_nested.not().then(|| quote! {#[automatically_derived]});

        let versioned_path = &*kubernetes_arguments.crates.versioned;
        let unknown_desired_api_version_error =
            quote! { #versioned_path::UnknownDesiredApiVersionError };

        // Get the per-version items to be able to iterate over them via quote
        let variant_strings = &tokens.variant_strings;
        let variant_idents = &tokens.variant_idents;
        let api_versions = variant_strings
            .iter()
            .map(|version| format!("{group}/{version}", group = &kubernetes_arguments.group))
            .collect::<Vec<_>>();

        quote! {
            #automatically_derived
            #vis enum #enum_ident {
                #(#variant_idents),*
            }

            #automatically_derived
            impl ::std::fmt::Display for #enum_ident {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::result::Result<(), ::std::fmt::Error> {
                    // The version (without the Kubernetes group) is probably more human-readable
                    f.write_str(self.as_version_str())
                }
            }

            #automatically_derived
            impl #enum_ident {
                pub fn as_version_str(&self) -> &str {
                    match self {
                        #(#variant_idents => #variant_strings),*
                    }
                }

                pub fn as_api_version_str(&self) -> &str {
                    match self {
                        #(#variant_idents => #api_versions),*
                    }
                }

                pub fn try_from_api_version(api_version: &str) -> Result<Self, #unknown_desired_api_version_error> {
                    match api_version {
                        #(#api_versions => Ok(Self::#variant_idents)),*,
                        _ => Err(#unknown_desired_api_version_error {
                            api_version: api_version.to_owned(),
                        }),
                    }
                }
            }
        }
    }

    /////////////////////////
    // CRD Conversion Code //
    /////////////////////////

    fn generate_kubernetes_status_struct(
        &self,
        kubernetes_arguments: &KubernetesArguments,
        is_nested: bool,
    ) -> Option<TokenStream> {
        kubernetes_arguments
            .options
            .experimental_conversion_tracking
            .is_present()
            .then(|| {
                let status_ident = &self.common.idents.kubernetes_status;

                let versioned_crate = &*kubernetes_arguments.crates.versioned;
                let schemars_crate = &*kubernetes_arguments.crates.schemars;
                let serde_crate = &*kubernetes_arguments.crates.serde;

                // Only add the #[automatically_derived] attribute if this impl is used outside of a
                // module (in standalone mode).
                let automatically_derived =
                    is_nested.not().then(|| quote! {#[automatically_derived]});

                // TODO (@Techassi): Validate that users don't specify the status we generate
                let status = kubernetes_arguments.status.as_ref().map(|status| {
                    quote! {
                        #[serde(flatten)]
                        pub status: #status,
                    }
                });

                quote! {
                    #automatically_derived
                    #[derive(
                        ::core::clone::Clone,
                        ::core::fmt::Debug,
                        #serde_crate::Deserialize,
                        #serde_crate::Serialize,
                        #schemars_crate::JsonSchema
                    )]
                    #[serde(rename_all = "camelCase")]
                    pub struct #status_ident {
                        pub changed_values: #versioned_crate::ChangedValues,

                        #status
                    }
                }
            })
    }

    fn generate_kubernetes_conversion(
        &self,
        versions: &[VersionDefinition],
    ) -> Option<TokenStream> {
        let kubernetes_arguments = self.common.options.kubernetes_arguments.as_ref()?;

        let struct_ident = &self.common.idents.kubernetes;
        let version_enum_ident = &self.common.idents.kubernetes_version;

        let kube_client_path = &*kubernetes_arguments.crates.kube_client;
        let serde_json_path = &*kubernetes_arguments.crates.serde_json;
        let versioned_path = &*kubernetes_arguments.crates.versioned;
        let kube_core_path = &*kubernetes_arguments.crates.kube_core;

        let convert_object_error = quote! { #versioned_path::ConvertObjectError };

        // Generate conversion paths and the match arms for these paths
        let conversion_match_arms =
            self.generate_kubernetes_conversion_match_arms(versions, kubernetes_arguments);
        let noop_match_arms = self.generate_kubernetes_noop_match_arms(versions);

        // TODO (@Techassi): Make this a feature, drop the option from the macro arguments
        // Generate tracing attributes and events if tracing is enabled
        let TracingTokens {
            successful_conversion_response_event,
            convert_objects_instrumentation,
            invalid_conversion_review_event,
            try_convert_instrumentation,
        } = self.generate_kubernetes_conversion_tracing(kubernetes_arguments);

        // Generate doc comments
        let conversion_review_reference =
            path_to_string(&parse_quote! { #kube_core_path::conversion::ConversionReview });

        let docs = formatdoc! {"
            Tries to convert a list of objects of kind [`{struct_ident}`] to the desired API version
            specified in the [`ConversionReview`][cr].

            The returned [`ConversionReview`][cr] either indicates a success or a failure, which
            is handed back to the Kubernetes API server.

            [cr]: {conversion_review_reference}"
        }
        .into_doc_comments();

        Some(quote! {
            #(#[doc = #docs])*
            #try_convert_instrumentation
            pub fn try_convert(review: #kube_core_path::conversion::ConversionReview)
                -> #kube_core_path::conversion::ConversionReview
            {
                // First, turn the review into a conversion request
                let request = match #kube_core_path::conversion::ConversionRequest::from_review(review) {
                    ::std::result::Result::Ok(request) => request,
                    ::std::result::Result::Err(err) => {
                        #invalid_conversion_review_event

                        return #kube_core_path::conversion::ConversionResponse::invalid(
                            #kube_client_path::Status {
                                status: Some(#kube_core_path::response::StatusSummary::Failure),
                                message: err.to_string(),
                                reason: err.to_string(),
                                details: None,
                                code: 400,
                            }
                        ).into_review()
                    }
                };

                // Convert all objects into the desired version
                let response = match Self::convert_objects(request.objects, &request.desired_api_version) {
                    ::std::result::Result::Ok(converted_objects) => {
                        #successful_conversion_response_event

                        // We construct the response from the ground up as the helper functions
                        // don't provide any benefit over manually doing it. Constructing a
                        // ConversionResponse via for_request is not possible due to a partial move
                        // of request.objects. The function internally doesn't even use the list of
                        // objects. The success function on ConversionResponse basically only sets
                        // the result to success and the converted objects to the provided list.
                        // The below code does the same thing.
                        #kube_core_path::conversion::ConversionResponse {
                            result: #kube_client_path::Status::success(),
                            types: request.types,
                            uid: request.uid,
                            converted_objects,
                        }
                    },
                    ::std::result::Result::Err(err) => {
                        let code = err.http_status_code();
                        let message = err.join_errors();

                        #kube_core_path::conversion::ConversionResponse {
                            result: #kube_client_path::Status {
                                status: Some(#kube_core_path::response::StatusSummary::Failure),
                                message: message.clone(),
                                reason: message,
                                details: None,
                                code,
                            },
                            types: request.types,
                            uid: request.uid,
                            converted_objects: vec![],
                        }
                    },
                };

                response.into_review()
            }

            #convert_objects_instrumentation
            fn convert_objects(
                objects: ::std::vec::Vec<#serde_json_path::Value>,
                desired_api_version: &str,
            )
                -> ::std::result::Result<::std::vec::Vec<#serde_json_path::Value>, #convert_object_error>
            {
                let desired_api_version = #version_enum_ident::try_from_api_version(desired_api_version)
                    .map_err(|source| #convert_object_error::ParseDesiredApiVersion { source })?;

                let mut converted_objects = ::std::vec::Vec::with_capacity(objects.len());

                for object in objects {
                    // This clone is required because in the noop case we move the object into
                    // the converted objects vec.
                    let current_object = Self::try_from_json_object(object.clone())
                        .map_err(|source| #convert_object_error::Parse { source })?;

                    match (current_object, &desired_api_version) {
                        #(#conversion_match_arms,)*
                        // We explicitly list the remaining no-op cases, so the compiler ensures we
                        // did not miss a conversion.
                        #(#noop_match_arms,)*
                    }
                }

                ::std::result::Result::Ok(converted_objects)
            }
        })
    }

    fn generate_kubernetes_conversion_match_arms(
        &self,
        versions: &[VersionDefinition],
        kubernetes_arguments: &KubernetesArguments,
    ) -> Vec<TokenStream> {
        let group = &kubernetes_arguments.group;
        let variant_data_ident = &self.common.idents.kubernetes_parameter;
        let struct_ident = &self.common.idents.kubernetes;
        let version_enum_ident = &self.common.idents.kubernetes_version;
        let spec_ident = &self.common.idents.original;

        let versioned_path = &*kubernetes_arguments.crates.versioned;
        let convert_object_error = quote! { #versioned_path::ConvertObjectError };

        let conversion_paths = conversion_paths(versions);

        conversion_paths
            .iter()
            .map(|(start, path)| {
                let current_object_version_ident = &start.idents.variant;
                let current_object_version_string = &start.inner.to_string();

                let desired_object_version = path.last().expect("the path always contains at least one element");
                let desired_object_api_version_string = format!(
                    "{group}/{desired_object_version}",
                    desired_object_version = desired_object_version.inner
                );
                let desired_object_variant_ident = &desired_object_version.idents.variant;
                let desired_object_module_ident = &desired_object_version.idents.module;

                let conversions = path.iter().enumerate().map(|(i, v)| {
                    let module_ident = &v.idents.module;

                    if i == 0 {
                        quote! {
                            let converted: #module_ident::#spec_ident = #variant_data_ident.spec.into();
                        }
                    } else {
                        quote! {
                            let converted: #module_ident::#spec_ident = converted.into();
                        }
                    }
                });

                let kind = self.common.idents.kubernetes.to_string();
                let steps = path.len();

                let convert_object_trace = kubernetes_arguments.options.enable_tracing.is_present().then(|| quote! {
                    ::tracing::trace!(
                        #DESIRED_API_VERSION_ATTRIBUTE = #desired_object_api_version_string,
                        #API_VERSION_ATTRIBUTE = #current_object_version_string,
                        #STEPS_ATTRIBUTE = #steps,
                        #KIND_ATTRIBUTE = #kind,
                        "Successfully converted object"
                    );
                });

                // Carry over the status field if the user set a status subresource
                let status_field = kubernetes_arguments.status
                    .is_some()
                    .then(|| quote! { status: #variant_data_ident.status, });

                quote! {
                    (Self::#current_object_version_ident(#variant_data_ident), #version_enum_ident::#desired_object_variant_ident) => {
                        #(#conversions)*

                        let desired_object = Self::#desired_object_variant_ident(#desired_object_module_ident::#struct_ident {
                            metadata: #variant_data_ident.metadata,
                            #status_field
                            spec: converted,
                        });

                        let desired_object = desired_object.into_json_value()
                            .map_err(|source| #convert_object_error::Serialize { source })?;

                        #convert_object_trace

                        converted_objects.push(desired_object);
                    }
                }
            })
            .collect()
    }

    fn generate_kubernetes_noop_match_arms(
        &self,
        versions: &[VersionDefinition],
    ) -> Vec<TokenStream> {
        let version_enum_ident = &self.common.idents.kubernetes_version;

        versions
            .iter()
            .map(|version| {
                let version_ident = &version.idents.variant;

                quote! {
                    // This is the case if the desired version matches the current object api version.
                    // NOTE (@Techassi): I'm curious if this will ever happen? In theory the K8s
                    // apiserver should never send such a conversion review.
                    (Self::#version_ident(_), #version_enum_ident::#version_ident) => converted_objects.push(object)
                }
            })
            .collect()
    }

    fn generate_kubernetes_conversion_tracing(
        &self,
        kubernetes_arguments: &KubernetesArguments,
    ) -> TracingTokens {
        if kubernetes_arguments.options.enable_tracing.is_present() {
            // TODO (@Techassi): Make tracing path configurable. Currently not possible, needs
            // upstream change
            let kind = self.common.idents.kubernetes.to_string();

            let successful_conversion_response_event = Some(quote! {
                ::tracing::debug!(
                    #CONVERTED_OBJECT_COUNT_ATTRIBUTE = converted_objects.len(),
                    #KIND_ATTRIBUTE = #kind,
                    "Successfully converted objects"
                );
            });

            let convert_objects_instrumentation = Some(quote! {
                #[::tracing::instrument(
                    skip_all,
                    err
                )]
            });

            let invalid_conversion_review_event = Some(quote! {
                ::tracing::warn!(?err, "received invalid conversion review");
            });

            // NOTE (@Techassi): We sadly cannot use the constants here, because
            // the fields only accept idents, which strings are not.
            let try_convert_instrumentation = Some(quote! {
                #[::tracing::instrument(
                    skip_all,
                    fields(
                        k8s.crd.conversion.api_version = review.types.api_version,
                        k8s.crd.kind = review.types.kind,
                    )
                )]
            });

            TracingTokens {
                successful_conversion_response_event,
                convert_objects_instrumentation,
                invalid_conversion_review_event,
                try_convert_instrumentation,
            }
        } else {
            TracingTokens::default()
        }
    }
}

#[derive(Debug, Default)]
struct TracingTokens {
    successful_conversion_response_event: Option<TokenStream>,
    convert_objects_instrumentation: Option<TokenStream>,
    invalid_conversion_review_event: Option<TokenStream>,
    try_convert_instrumentation: Option<TokenStream>,
}

fn conversion_paths<T>(elements: &[T]) -> Vec<(&T, Cow<'_, [T]>)>
where
    T: Clone + Ord,
{
    let mut chain = Vec::new();

    // First, create all 2-permutations of the provided list of elements. It is important
    // we select permutations instead of combinations because the order of elements matter.
    // A quick example of what the iterator adaptor produces: A list with three elements
    // 'v1alpha1', 'v1beta1', and 'v1' will produce six (3! / (3 - 2)!) permutations:
    //
    // - v1alpha1 -> v1beta1
    // - v1alpha1 -> v1
    // - v1beta1  -> v1
    // - v1beta1  -> v1alpha1
    // - v1       -> v1alpha1
    // - v1       -> v1beta1

    for pair in elements.iter().permutations(2) {
        let start = pair[0];
        let end = pair[1];

        // Next, we select the positions of the start and end element in the original
        // slice. These indices are used to construct the conversion path, which contains
        // elements between start (excluding) and the end (including). These elements
        // describe the steps needed to go from the start to the end (upgrade or downgrade
        // depending on the direction).
        if let (Some(start_index), Some(end_index)) = (
            elements.iter().position(|v| v == start),
            elements.iter().position(|v| v == end),
        ) {
            let path = match start_index.cmp(&end_index) {
                Ordering::Less => {
                    // If the start index is smaller than the end index (upgrade), we can return
                    // a slice pointing directly into the original slice. That's why Cow::Borrowed
                    // can be used here.
                    Cow::Borrowed(&elements[start_index + 1..=end_index])
                }
                Ordering::Greater => {
                    // If the start index is bigger than the end index (downgrade), we need to reverse
                    // the elements. With a slice, this is only possible to do in place, which is not
                    // what we want in this case. Instead, the data is reversed and cloned and collected
                    // into a Vec and Cow::Owned is used.
                    let path = elements[end_index..start_index]
                        .iter()
                        .rev()
                        .cloned()
                        .collect();
                    Cow::Owned(path)
                }
                Ordering::Equal => unreachable!(
                    "start and end index cannot be the same due to selecting permutations"
                ),
            };

            chain.push((start, path));
        }
    }

    chain
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_path_is_the_goal() {
        let paths = conversion_paths(&["v1alpha1", "v1alpha2", "v1beta1", "v1"]);
        assert_eq!(paths.len(), 12);

        let expected = vec![
            ("v1alpha1", vec!["v1alpha2"]),
            ("v1alpha1", vec!["v1alpha2", "v1beta1"]),
            ("v1alpha1", vec!["v1alpha2", "v1beta1", "v1"]),
            ("v1alpha2", vec!["v1alpha1"]),
            ("v1alpha2", vec!["v1beta1"]),
            ("v1alpha2", vec!["v1beta1", "v1"]),
            ("v1beta1", vec!["v1alpha2", "v1alpha1"]),
            ("v1beta1", vec!["v1alpha2"]),
            ("v1beta1", vec!["v1"]),
            ("v1", vec!["v1beta1", "v1alpha2", "v1alpha1"]),
            ("v1", vec!["v1beta1", "v1alpha2"]),
            ("v1", vec!["v1beta1"]),
        ];

        for (result, expected) in paths.iter().zip(expected) {
            assert_eq!(*result.0, expected.0);
            assert_eq!(result.1.to_vec(), expected.1);
        }
    }
}
