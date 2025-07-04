use std::{borrow::Cow, cmp::Ordering};

use indoc::formatdoc;
use itertools::Itertools as _;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse_quote;

use crate::{
    codegen::{
        Direction, VersionContext, VersionDefinition,
        changes::Neighbors as _,
        container::r#struct::{SpecGenerationContext, Struct},
        item::ItemStatus,
        module::ModuleGenerationContext,
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
    /// Generates the Kubernetes specific From impl for the top-level object.
    pub(super) fn generate_object_from_impl(
        &self,
        direction: Direction,
        ver_ctx: VersionContext<'_>,
        mod_gen_ctx: ModuleGenerationContext<'_>,
        spec_gen_ctx: &SpecGenerationContext<'_>,
    ) -> Option<TokenStream> {
        if mod_gen_ctx.skip.object_from.is_present() || self.common.options.skip_object_from {
            return None;
        }

        let next_version = ver_ctx.next_version;
        let version = ver_ctx.version;

        next_version.map(|next_version| {
            let from_struct_parameter_ident = &spec_gen_ctx.kubernetes_idents.parameter;
            let object_struct_ident = &spec_gen_ctx.kubernetes_idents.kind;
            let spec_struct_ident = &self.common.idents.original;

            let versioned_path = &*mod_gen_ctx.crates.versioned;

            let automatically_derived = mod_gen_ctx.automatically_derived_attr();

            let (for_module_ident, from_module_ident) = match direction {
                Direction::Upgrade => (&next_version.idents.module, &version.idents.module),
                Direction::Downgrade => (&version.idents.module, &next_version.idents.module),
            };

            // NOTE (@Techassi): This if statement can be removed once experimental_conversion_tracking
            // is gone.
            let from_inner = if mod_gen_ctx.kubernetes_options.experimental_conversion_tracking.is_present() {
                quote! {
                    // The status is optional. The be able to track changes in nested sub structs it needs
                    // to be initialized with a default value.
                    let mut status = #from_struct_parameter_ident.status.unwrap_or_default();

                    // Convert the spec and track values in the status
                    let spec =
                        <#for_module_ident::#spec_struct_ident as #versioned_path::TrackingFrom<_, _>>::tracking_from(
                            #from_struct_parameter_ident.spec,
                            &mut status,
                            "",
                        );

                    // Construct the final object by copying over the metadata, setting the status and
                    // using the converted spec.
                    Self {
                        metadata: #from_struct_parameter_ident.metadata,
                        status: Some(status),
                        spec,
                    }
                }
            } else {
                let status = spec_gen_ctx.kubernetes_arguments.status
                    .as_ref()
                    .map(|_| quote! { status:  #from_struct_parameter_ident.status,});

                quote! {
                    Self {
                        metadata: #from_struct_parameter_ident.metadata,
                        spec: #from_struct_parameter_ident.spec.into(),
                        #status
                    }
                }
            };

            quote! {
                #automatically_derived
                impl ::std::convert::From<#from_module_ident::#object_struct_ident> for #for_module_ident::#object_struct_ident {
                    fn from(#from_struct_parameter_ident: #from_module_ident::#object_struct_ident) -> Self {
                        #from_inner
                    }
                }
            }
        })
    }

    pub(super) fn generate_tracking_from_impl(
        &self,
        direction: Direction,
        version: &VersionDefinition,
        next_version: &VersionDefinition,
        mod_gen_ctx: ModuleGenerationContext<'_>,
    ) -> TokenStream {
        // TODO (@Techassi): Support generic types which have been removed in newer versions,
        // but need to exist for older versions How do we represent that? Because the
        // defined struct always represents the latest version. I guess we could generally
        // advise against using generic types, but if you have to, avoid removing it in
        // later versions.
        let from_struct_ident = &self.common.idents.parameter;
        let struct_ident = &self.common.idents.original;

        let versioned_path = &*mod_gen_ctx.crates.versioned;

        // Include allow(deprecated) only when this or the next version is
        // deprecated. Also include it, when a field in this or the next
        // version is deprecated.
        let allow_attribute = (version.deprecated.is_some()
            || next_version.deprecated.is_some()
            || self.is_any_field_deprecated(version)
            || self.is_any_field_deprecated(next_version))
        .then(|| quote! { #[allow(deprecated)] });

        // Only add the #[automatically_derived] attribute only if this impl is used
        // outside of a module (in standalone mode).
        let automatically_derived = mod_gen_ctx.automatically_derived_attr();

        let fields = |direction: Direction| -> TokenStream {
            self.fields
                .iter()
                .filter_map(|f| {
                    f.generate_for_from_impl(direction, version, next_version, from_struct_ident)
                })
                .collect()
        };

        let (fields, for_module_ident, from_module_ident) = match direction {
            direction @ Direction::Upgrade => {
                let from_module_ident = &version.idents.module;
                let for_module_ident = &next_version.idents.module;

                (fields(direction), for_module_ident, from_module_ident)
            }
            direction @ Direction::Downgrade => {
                let from_module_ident = &next_version.idents.module;
                let for_module_ident = &version.idents.module;

                (fields(direction), for_module_ident, from_module_ident)
            }
        };

        let inserts = self.generate_tracking_inserts(direction, next_version, mod_gen_ctx);
        let removals = self.generate_tracking_removals(direction, next_version, mod_gen_ctx);
        let json_paths = self.generate_json_paths(next_version);

        // TODO (@Techassi): Re-add support for generics
        // TODO (@Techassi): We know the status, so we can hard-code it, but hard to track across structs

        quote! {
            #automatically_derived
            #allow_attribute
            impl<S> #versioned_path::TrackingFrom<#from_module_ident::#struct_ident, S> for #for_module_ident::#struct_ident
            where
                S: #versioned_path::TrackingStatus + ::core::default::Default
            {
                fn tracking_from(#from_struct_ident: #from_module_ident::#struct_ident, status: &mut S, parent: &str) -> Self {
                    // TODO (@Techassi): Only emit this if any of the fields below need it
                    use #versioned_path::TrackingInto as _;

                    #json_paths
                    // Depending on the direction, we need to either insert changed values into
                    // the upgrade or downgrade section. Only then we can convert the spec.
                    #inserts

                    let mut spec = Self {
                        #fields
                    };

                    // After the spec is converted, depending on the direction, we need to apply
                    // changed values from either the upgrade or downgrade section. Afterwards
                    // we can return the successfully converted spec and the status contains
                    // the tracked changes.
                    #removals
                    spec
                }
            }
        }
    }

    fn generate_tracking_inserts(
        &self,
        direction: Direction,
        next_version: &VersionDefinition,
        mod_gen_ctx: ModuleGenerationContext<'_>,
    ) -> Option<TokenStream> {
        if !self.needs_tracking(next_version) {
            return None;
        }

        match direction {
            // This is only needed once we support removal of fields
            Direction::Upgrade => None,
            Direction::Downgrade => {
                let next_version_string = next_version.inner.to_string();
                let from_struct_ident = &self.common.idents.parameter;

                let inserts: TokenStream = self
                    .fields
                    .iter()
                    .filter_map(|f| {
                        f.generate_for_status_insertion(
                            direction,
                            next_version,
                            from_struct_ident,
                            mod_gen_ctx,
                        )
                    })
                    .collect();

                Some(quote! {
                    let upgrades = status
                        .changes()
                        .upgrades
                        .entry(#next_version_string.to_owned())
                        .or_default();

                    #inserts
                })
            }
        }
    }

    fn generate_tracking_removals(
        &self,
        direction: Direction,
        next_version: &VersionDefinition,
        mod_gen_ctx: ModuleGenerationContext<'_>,
    ) -> Option<TokenStream> {
        if !self.needs_tracking(next_version) {
            return None;
        }

        let match_arms: TokenStream = self
            .fields
            .iter()
            .filter_map(|f| f.generate_for_status_removal(direction, next_version))
            .collect();

        match direction {
            Direction::Upgrade => {
                let next_version_string = next_version.inner.to_string();
                let versioned_path = &*mod_gen_ctx.crates.versioned;

                Some(quote! {
                    // NOTE (@Techassi): This is an awkward thing to do. Can we possibly use &str for the keys here?
                    if let Some(upgrades) = status.changes().upgrades.remove(&#next_version_string.to_owned()) {
                        for #versioned_path::ChangedValue { field_name, value } in upgrades {
                            match field_name {
                                #match_arms
                                _ => unreachable!(),
                            }
                        }
                    }
                })
            }
            // This is only needed once we support removal of fields
            Direction::Downgrade => None,
        }
    }

    fn generate_json_paths(&self, next_version: &VersionDefinition) -> Option<TokenStream> {
        let json_paths = self
            .fields
            .iter()
            .filter_map(|f| f.generate_for_json_path(next_version))
            .collect();

        Some(json_paths)
    }

    pub(super) fn needs_tracking(&self, version: &VersionDefinition) -> bool {
        self.fields.iter().any(|f| {
            f.changes.as_ref().is_some_and(|c| {
                c.value_is(&version.inner, |s| {
                    // For now, only added fields need to be tracked. In the future, removals and
                    // type changes also need to be tracked
                    matches!(s, ItemStatus::Addition { .. })
                })
            })
        })
    }

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
        let match_arms = self.generate_conversion_match_arms(versions, mod_gen_ctx, spec_gen_ctx);

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
                    let current_object = Self::from_json_value(object.clone())
                        .map_err(|source| #convert_object_error::Parse { source })?;

                    match (current_object, desired_api_version) {
                        #(#match_arms,)*
                        // If no match arm matches, this is a noop. This is the case if the desired
                        // version matches the current object api version.
                        // NOTE (@Techassi): I'm curious if this will ever happen? In theory the K8s
                        // apiserver should never send such a conversion review.
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

        if !mod_gen_ctx
            .kubernetes_options
            .experimental_conversion_tracking
            .is_present()
        {
            return None;
        }

        let status_ident = &spec_gen_ctx.kubernetes_idents.status;

        let versioned_path = &*mod_gen_ctx.crates.versioned;
        let schemars_path = &*mod_gen_ctx.crates.schemars;
        let serde_path = &*mod_gen_ctx.crates.serde;

        let automatically_derived = mod_gen_ctx.automatically_derived_attr();

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
            #automatically_derived
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

            #automatically_derived
            impl #versioned_path::TrackingStatus for #status_ident {
                fn changes(&mut self) -> &mut #versioned_path::ChangedValues {
                    &mut self.changed_values
                }
            }
        })
    }

    pub(super) fn generate_from_json_value_fn(
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

        let version_strings = &spec_gen_ctx.version_strings;
        let variant_idents = &spec_gen_ctx.variant_idents;

        let enum_ident_string = spec_gen_ctx.kubernetes_idents.kind.to_string();

        let api_versions = version_strings.iter().map(|version| {
            format!(
                "{group}/{version}",
                group = &spec_gen_ctx.kubernetes_arguments.group
            )
        });

        Some(quote! {
            fn from_json_value(object_value: #serde_json_path::Value) -> ::std::result::Result<Self, #parse_object_error> {
                let kind = object_value
                    .get("kind")
                    .ok_or_else(|| #parse_object_error::FieldNotPresent {
                        field: "kind".to_owned()
                    })?
                    .as_str()
                    .ok_or_else(|| #parse_object_error::FieldNotStr {
                        field: "kind".to_owned()
                    })?;

                if kind == #enum_ident_string {
                    return Err(#parse_object_error::UnexpectedKind{
                        kind: kind.to_owned(),
                        expected: #enum_ident_string.to_owned(),
                    });
                }

                let api_version = object_value
                    .get("apiVersion")
                    .ok_or_else(|| #parse_object_error::FieldNotPresent {
                        field: "apiVersion".to_owned()
                    })?
                    .as_str()
                    .ok_or_else(|| #parse_object_error::FieldNotStr {
                        field: "apiVersion".to_owned()
                    })?;

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
        })
    }

    pub(super) fn generate_into_json_value_fn(
        &self,
        mod_gen_ctx: ModuleGenerationContext<'_>,
        spec_gen_ctx: &SpecGenerationContext<'_>,
    ) -> Option<TokenStream> {
        if mod_gen_ctx.skip.try_convert.is_present() || self.common.options.skip_try_convert {
            return None;
        }

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
                        #DESIRED_API_VERSION_ATTRIBUTE = #desired_object_version_string,
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
                        k8s.crd.conversion.kind = review.types.kind,
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
