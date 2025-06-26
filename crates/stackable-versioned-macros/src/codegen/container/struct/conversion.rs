use std::{borrow::Cow, cmp::Ordering};

use indoc::formatdoc;
use itertools::Itertools as _;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse_quote;

use crate::{
    codegen::{
        VersionDefinition,
        container::{
            ModuleGenerationContext,
            r#struct::{SpecGenerationContext, Struct},
        },
    },
    utils::{doc_comments::DocComments as _, path_to_string},
};

const CONVERTED_OBJECT_COUNT_ATTRIBUTE: &str = "k8s.crd.conversion.converted_object_count";
const DESIRED_API_VERSION_ATTRIBUTE: &str = "k8s.crd.conversion.desired_api_version";
const API_VERSION_ATTRIBUTE: &str = "k8s.crd.conversion.api_version";
const STEPS_ATTRIBUTE: &str = "k8s.crd.conversion.steps";
const KIND_ATTRIBUTE: &str = "k8s.crd.conversion.kind";

#[derive(Debug, Default)]
pub struct TracingTokens {
    pub successful_conversion_response_event: Option<TokenStream>,
    pub convert_objects_instrumentation: Option<TokenStream>,
    pub invalid_conversion_review_event: Option<TokenStream>,
    pub try_convert_instrumentation: Option<TokenStream>,
}

impl Struct {
    pub(super) fn generate_try_convert_fn(
        &self,
        versions: &[VersionDefinition],
        mod_gen_ctx: ModuleGenerationContext<'_>,
        spec_gen_ctx: &SpecGenerationContext<'_>,
    ) -> Option<TokenStream> {
        if mod_gen_ctx.skip.try_convert.is_present() || self.common.options.skip_try_convert {
            return None;
        }

        let version_enum_ident = &spec_gen_ctx.kubernetes_idents.version;
        let struct_ident = &spec_gen_ctx.kubernetes_idents.kind;

        let kube_client_path = &*mod_gen_ctx.crates.kube_client;
        let serde_json_path = &*mod_gen_ctx.crates.serde_json;
        let kube_core_path = &*mod_gen_ctx.crates.kube_core;
        let versioned_path = &*mod_gen_ctx.crates.versioned;

        let convert_object_error = quote! { #versioned_path::ConvertObjectError };

        // Generate conversion paths and the match arms for these paths
        let conversion_match_arms =
            self.generate_conversion_match_arms(versions, mod_gen_ctx, spec_gen_ctx);

        // TODO (@Techassi): Make this a feature, drop the option from the macro arguments
        // Generate tracing attributes and events if tracing is enabled
        let TracingTokens {
            successful_conversion_response_event,
            convert_objects_instrumentation,
            invalid_conversion_review_event,
            try_convert_instrumentation,
        } = self.generate_conversion_tracing(mod_gen_ctx, spec_gen_ctx);

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
                let desired_api_version = #version_enum_ident::from_api_version(desired_api_version)
                    .map_err(|source| #convert_object_error::ParseDesiredApiVersion { source })?;

                let mut converted_objects = ::std::vec::Vec::with_capacity(objects.len());

                for object in objects {
                    // This clone is required because in the noop case we move the object into
                    // the converted objects vec.
                    let current_object = Self::from_json_object(object.clone())
                        .map_err(|source| #convert_object_error::Parse { source })?;

                    match (current_object, desired_api_version) {
                        #(#conversion_match_arms,)*
                        // In case the desired version matches the current object api version, we
                        // don't need to do anything.
                        //
                        // NOTE (@Techassi): I'm curious if this will ever happen? In theory the K8s
                        // apiserver should never send such a conversion review.
                        //
                        // Note(@sbernauer): I would prefer to explicitly list the remaining no-op
                        // cases, so the compiler ensures we did not miss a conversion
                        // // let version_idents = versions.iter().map(|v| &v.idents.variant);
                        // #(
                        //     (Self::#version_idents(_), #version_enum_ident::#version_idents)
                        // )|* => converted_objects.push(object)
                        _ => converted_objects.push(object),
                    }
                }

                ::std::result::Result::Ok(converted_objects)
            }
        })
    }

    pub(super) fn generate_status_struct(
        &self,
        mod_gen_ctx: ModuleGenerationContext<'_>,
        spec_gen_ctx: &SpecGenerationContext<'_>,
    ) -> Option<TokenStream> {
        if mod_gen_ctx.skip.try_convert.is_present() || self.common.options.skip_try_convert {
            return None;
        }

        let status_ident = &spec_gen_ctx.kubernetes_idents.status;

        let versioned_path = &*mod_gen_ctx.crates.versioned;
        let schemars_path = &*mod_gen_ctx.crates.schemars;
        let serde_path = &*mod_gen_ctx.crates.serde;

        // TODO (@Techassi): Validate that users don't specify the status we generate
        let status = spec_gen_ctx
            .kubernetes_arguments
            .status
            .as_ref()
            .map(|status| {
                quote! {
                    #[serde(flatten)]
                    pub status: #status,
                }
            });

        Some(quote! {
            #[derive(
                ::core::clone::Clone,
                ::core::default::Default,
                ::core::fmt::Debug,
                #serde_path::Deserialize,
                #serde_path::Serialize,
                #schemars_path::JsonSchema
            )]
            #[serde(rename_all = "camelCase")]
            pub struct #status_ident {
                pub changed_values: #versioned_path::ChangedValues,

                #status
            }

            impl #versioned_path::TrackingStatus for #status_ident {
                fn changes(&mut self) -> &mut #versioned_path::ChangedValues {
                    &mut self.changed_values
                }
            }
        })
    }

    pub(super) fn generate_from_json_object_fn(
        &self,
        mod_gen_ctx: ModuleGenerationContext<'_>,
        spec_gen_ctx: &SpecGenerationContext<'_>,
    ) -> Option<TokenStream> {
        if mod_gen_ctx.skip.try_convert.is_present() || self.common.options.skip_try_convert {
            return None;
        }

        let serde_json_path = &*mod_gen_ctx.crates.serde_json;
        let versioned_path = &*mod_gen_ctx.crates.versioned;

        let parse_object_error = quote! { #versioned_path::ParseObjectError };
        let enum_ident_string = spec_gen_ctx.kubernetes_idents.kind.to_string();

        let version_strings = &spec_gen_ctx.version_strings;
        let variant_idents = &spec_gen_ctx.variant_idents;

        let api_versions = version_strings.iter().map(|version| {
            format!(
                "{group}/{version}",
                group = &spec_gen_ctx.kubernetes_arguments.group
            )
        });

        Some(quote! {
            fn from_json_object(object_value: #serde_json_path::Value) -> ::std::result::Result<Self, #parse_object_error> {
                let object_kind = object_value
                    .get("kind")
                    .ok_or_else(|| #parse_object_error::FieldMissing{ field: "kind".to_owned() })?
                    .as_str()
                    .ok_or_else(|| #parse_object_error::FieldNotStr{ field: "kind".to_owned() })?;

                // Note(@sbernauer): The kind must be checked here, because it is possible for the
                // wrong object to be deserialized. Checking here stops us assuming the kind is
                // correct and accidentally updating upgrade/downgrade information in the status in
                // a later step.
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
                    #(#api_versions  => {
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
        })
    }

    pub(super) fn generate_into_json_value_fn(
        &self,
        mod_gen_ctx: ModuleGenerationContext<'_>,
        spec_gen_ctx: &SpecGenerationContext<'_>,
    ) -> Option<TokenStream> {
        let variant_data_ident = &spec_gen_ctx.kubernetes_idents.parameter;
        let variant_idents = &spec_gen_ctx.variant_idents;

        let serde_json_path = &*mod_gen_ctx.crates.serde_json;

        Some(quote! {
            fn into_json_value(self) -> ::std::result::Result<#serde_json_path::Value, #serde_json_path::Error> {
                match self {
                    #(Self::#variant_idents(#variant_data_ident) => Ok(#serde_json_path::to_value(#variant_data_ident)?),)*
                }
            }
        })
    }

    fn generate_conversion_match_arms(
        &self,
        versions: &[VersionDefinition],
        mod_gen_ctx: ModuleGenerationContext<'_>,
        spec_gen_ctx: &SpecGenerationContext<'_>,
    ) -> Vec<TokenStream> {
        let variant_data_ident = &spec_gen_ctx.kubernetes_idents.parameter;
        let version_enum_ident = &spec_gen_ctx.kubernetes_idents.version;
        let struct_ident = &spec_gen_ctx.kubernetes_idents.kind;

        let versioned_path = &*mod_gen_ctx.crates.versioned;
        let convert_object_error = quote! { #versioned_path::ConvertObjectError };

        let conversion_paths = conversion_paths(versions);

        conversion_paths
            .iter()
            .map(|(start, path)| {
                let current_object_version_ident = &start.idents.variant;
                let current_object_version_string = &start.inner.to_string();

                let desired_object_version = path.last().expect("the path always contains at least one element");
                let desired_object_version_string = desired_object_version.inner.to_string();
                let desired_object_api_version_string = format!(
                    "{group}/{desired_object_version_string}",
                    group = spec_gen_ctx.kubernetes_arguments.group
                );
                let desired_object_variant_ident = &desired_object_version.idents.variant;

                let conversions = path.iter().enumerate().map(|(i, v)| {
                    let module_ident = &v.idents.module;

                    if i == 0 {
                        quote! {
                            // let converted: #module_ident::#spec_ident = #variant_data_ident.spec.into();
                            let converted: #module_ident::#struct_ident = #variant_data_ident.into();
                        }
                    } else {
                        quote! {
                            // let converted: #module_ident::#spec_ident = converted.into();
                            let converted: #module_ident::#struct_ident = converted.into();
                        }
                    }
                });

                let kind = spec_gen_ctx.kubernetes_idents.kind.to_string();
                let steps = path.len();

                let convert_object_trace = mod_gen_ctx.kubernetes_options.enable_tracing.is_present().then(|| quote! {
                    ::tracing::trace!(
                        #DESIRED_API_VERSION_ATTRIBUTE = #desired_object_api_version_string,
                        #API_VERSION_ATTRIBUTE = #current_object_version_string,
                        #STEPS_ATTRIBUTE = #steps,
                        #KIND_ATTRIBUTE = #kind,
                        "Successfully converted object"
                    );
                });


                quote! {
                    (Self::#current_object_version_ident(#variant_data_ident), #version_enum_ident::#desired_object_variant_ident) => {
                        #(#conversions)*

                        let desired_object = Self::#desired_object_variant_ident(converted);

                        let desired_object = desired_object.into_json_value()
                            .map_err(|source| #convert_object_error::Serialize { source })?;

                        #convert_object_trace

                        converted_objects.push(desired_object);
                    }
                }
            })
            .collect()
    }

    fn generate_conversion_tracing(
        &self,
        mod_gen_ctx: ModuleGenerationContext<'_>,
        spec_gen_ctx: &SpecGenerationContext<'_>,
    ) -> TracingTokens {
        if mod_gen_ctx.kubernetes_options.enable_tracing.is_present() {
            // TODO (@Techassi): Make tracing path configurable. Currently not possible, needs
            // upstream change
            let kind = spec_gen_ctx.kubernetes_idents.kind.to_string();

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
