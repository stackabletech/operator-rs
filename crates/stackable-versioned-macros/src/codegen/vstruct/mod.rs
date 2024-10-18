use std::ops::Deref;

use darling::util::IdentString;
use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_quote, Error, Fields, Ident};

use crate::{
    attrs::common::StandaloneContainerAttributes,
    codegen::{
        chain::Neighbors,
        common::{
            generate_module, Container, ContainerInput, Item, ItemStatus, VersionDefinition,
            VersionExt, VersionedContainer,
        },
        vstruct::field::VersionedField,
    },
};

pub(crate) mod field;

// NOTE (@Techassi): The generate_version function should return a triple of values. The first
// value is the complete container definition without any module wrapping it. The second value
// contains the generated tokens for the conversion between this version and the next one. Lastly,
// the third value contains Kubernetes related code. The last top values are wrapped in Option.
// type GenerateVersionReturn = (TokenStream, Option<TokenStream>, Option<KubernetesTokens>);
// type KubernetesTokens = (TokenStream, Ident, String);

pub(crate) struct GenerateVersionTokens {
    kubernetes_definition: Option<KubernetesTokens>,
    struct_definition: TokenStream,
    from_impl: Option<TokenStream>,
}

pub(crate) struct KubernetesTokens {
    merged_crd_fn_call: TokenStream,
    variant_display: String,
    enum_variant: Ident,
}

/// Stores individual versions of a single struct. Each version tracks field
/// actions, which describe if the field was added, renamed or deprecated in
/// that version. Fields which are not versioned, are included in every
/// version of the struct.
#[derive(Debug)]
pub(crate) struct VersionedStruct(VersionedContainer<VersionedField>);

impl Deref for VersionedStruct {
    type Target = VersionedContainer<VersionedField>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Container<Fields, VersionedField> for VersionedStruct {
    fn new(
        input: ContainerInput,
        fields: Fields,
        attributes: StandaloneContainerAttributes,
    ) -> syn::Result<Self> {
        let ident = &input.ident;

        // Convert the raw version attributes into a container version.
        let versions: Vec<_> = (&attributes).into();

        // Extract the field attributes for every field from the raw token
        // stream and also validate that each field action version uses a
        // version declared by the container attribute.
        let mut items = Vec::new();

        for field in fields {
            let mut versioned_field = VersionedField::new(field, &attributes)?;
            versioned_field.insert_container_versions(&versions);
            items.push(versioned_field);
        }

        // Check for field ident collisions
        for version in &versions {
            // Collect the idents of all fields for a single version and then
            // ensure that all idents are unique. If they are not, return an
            // error.

            // TODO (@Techassi): Report which field(s) use a duplicate ident and
            // also hint what can be done to fix it based on the field action /
            // status.

            if !items.iter().map(|f| f.get_ident(version)).all_unique() {
                return Err(Error::new(
                    ident.span(),
                    format!("struct contains renamed fields which collide with other fields in version {version}", version = version.inner),
                ));
            }
        }

        // Validate K8s specific requirements
        // Ensure that the struct name includes the 'Spec' suffix.
        if attributes.kubernetes_args.is_some() && !ident.to_string().ends_with("Spec") {
            return Err(Error::new(
                ident.span(),
                "struct name needs to include the `Spec` suffix if Kubernetes features are enabled via `#[versioned(k8s())]`"
            ));
        }

        Ok(Self(VersionedContainer::new(
            input, attributes, versions, items,
        )))
    }

    fn generate_standalone_tokens(&self) -> TokenStream {
        let mut kubernetes_definitions = Vec::new();
        let mut tokens = TokenStream::new();

        let mut versions = self.versions.iter().peekable();

        while let Some(version) = versions.next() {
            // Generate the container definition, from implementation and Kubernetes related tokens
            // for that particular version.
            let GenerateVersionTokens {
                struct_definition,
                from_impl,
                kubernetes_definition,
            } = self.generate_version(version, versions.peek().copied());

            let module_definition = generate_module(version, &self.visibility, struct_definition);

            if let Some(kubernetes_definition) = kubernetes_definition {
                kubernetes_definitions.push(kubernetes_definition);
            }

            tokens.extend(module_definition);
            tokens.extend(from_impl);
        }

        if !kubernetes_definitions.is_empty() {
            tokens.extend(self.generate_kubernetes_merge_crds(kubernetes_definitions));
        }

        tokens
    }

    fn generate_nested_tokens(&self) -> TokenStream {
        quote! {}
    }
}

impl VersionedStruct {
    /// Generates all tokens for a single instance of a versioned struct.
    ///
    /// This functions returns a value triple containing various pieces of generated code which can
    /// be combined in multiple ways to allow generate the correct code based on which mode we are
    /// running: "standalone" or "nested".
    ///
    /// # Struct Definition
    ///
    /// The first value of the triple contains the struct definition including all attributes and
    /// macros it needs. These tokens **do not** include the wrapping module indicating to which
    /// version this definition belongs. This is done deliberately to enable grouping multiple
    /// versioned containers when running in "nested" mode.
    ///
    /// # From Implementation
    ///
    /// The second value contains the [`From`] implementation which enables conversion from _this_
    /// version to the _next_ one. These tokens need to be placed outside the version modules,
    /// because they reference the structs using the version modules, like `v1alpha1` and `v1beta1`.
    ///
    /// # Kubernetes-specific Code
    ///
    /// The last value contains Kubernetes specific data. Currently, it contains data to generate
    /// code to enable merging CRDs.
    fn generate_version(
        &self,
        version: &VersionDefinition,
        next_version: Option<&VersionDefinition>,
    ) -> GenerateVersionTokens {
        let original_attributes = &self.original_attributes;
        let struct_name = &self.idents.original;

        // Generate fields of the struct for `version`.
        let fields = self.generate_struct_fields(version);

        // Generate doc comments for the container (struct)
        let version_specific_docs = self.generate_struct_docs(version);

        // Generate K8s specific code
        let (kubernetes_cr_derive, kubernetes_definition) = match &self.options.kubernetes_options {
            Some(options) => {
                // Generate the CustomResource derive macro with the appropriate
                // attributes supplied using #[kube()].
                let cr_derive = self.generate_kubernetes_cr_derive(version);

                // Generate merged_crd specific code when not opted out.
                let merged_crd = if !options.skip_merged_crd {
                    let merged_crd_fn_call = self.generate_kubernetes_crd_fn_call(version);
                    let enum_variant = version.inner.as_variant_ident();
                    let variant_display = version.inner.to_string();

                    Some(KubernetesTokens {
                        merged_crd_fn_call,
                        variant_display,
                        enum_variant,
                    })
                } else {
                    None
                };

                (Some(cr_derive), merged_crd)
            }
            None => (None, None),
        };

        // Generate struct definition tokens
        let struct_definition = quote! {
            #version_specific_docs
            #(#original_attributes)*
            #kubernetes_cr_derive
            pub struct #struct_name {
                #fields
            }
        };

        // Generate the From impl between this `version` and the next one.
        let from_impl = if !self.options.skip_from && !version.skip_from {
            self.generate_from_impl(version, next_version)
        } else {
            None
        };

        GenerateVersionTokens {
            kubernetes_definition,
            struct_definition,
            from_impl,
        }
    }

    /// Generates version specific doc comments for the struct.
    fn generate_struct_docs(&self, version: &VersionDefinition) -> TokenStream {
        let mut tokens = TokenStream::new();

        for (i, doc) in version.version_specific_docs.iter().enumerate() {
            if i == 0 {
                // Prepend an empty line to clearly separate the version
                // specific docs.
                tokens.extend(quote! {
                    #[doc = ""]
                })
            }
            tokens.extend(quote! {
                #[doc = #doc]
            })
        }

        tokens
    }

    /// Generates struct fields following the `name: type` format which includes
    /// a trailing comma.
    fn generate_struct_fields(&self, version: &VersionDefinition) -> TokenStream {
        let mut tokens = TokenStream::new();

        for item in &self.items {
            tokens.extend(item.generate_for_container(version));
        }

        tokens
    }

    /// Generates the [`From`] impl which enables conversion between a version
    /// and the next one.
    fn generate_from_impl(
        &self,
        version: &VersionDefinition,
        next_version: Option<&VersionDefinition>,
    ) -> Option<TokenStream> {
        if let Some(next_version) = next_version {
            let next_module_name = &next_version.ident;
            let module_name = &version.ident;

            let struct_ident = &self.idents.original;
            let from_ident = &self.idents.from;

            let fields = self.generate_from_fields(version, next_version, from_ident);

            // Include allow(deprecated) only when this or the next version is
            // deprecated. Also include it, when a field in this or the next
            // version is deprecated.
            let allow_attribute = (version.deprecated
                || next_version.deprecated
                || self.is_any_field_deprecated(version)
                || self.is_any_field_deprecated(next_version))
            .then_some(quote! { #[allow(deprecated)] });

            return Some(quote! {
                #[automatically_derived]
                #allow_attribute
                impl ::std::convert::From<#module_name::#struct_ident> for #next_module_name::#struct_ident {
                    fn from(#from_ident: #module_name::#struct_ident) -> Self {
                        Self {
                            #fields
                        }
                    }
                }
            });
        }

        None
    }

    /// Generates fields used in the [`From`] impl following the
    /// `new_name: struct_name.old_name` format which includes a trailing comma.
    fn generate_from_fields(
        &self,
        version: &VersionDefinition,
        next_version: &VersionDefinition,
        from_ident: &IdentString,
    ) -> TokenStream {
        let mut token_stream = TokenStream::new();

        for item in &self.items {
            token_stream.extend(item.generate_for_from_impl(version, next_version, from_ident))
        }

        token_stream
    }

    /// Returns whether any field is deprecated in the provided
    /// [`ContainerVersion`].
    fn is_any_field_deprecated(&self, version: &VersionDefinition) -> bool {
        // First, iterate over all fields. Any will return true if any of the
        // function invocations return true. If a field doesn't have a chain,
        // we can safely default to false (unversioned fields cannot be
        // deprecated). Then we retrieve the status of the field and ensure it
        // is deprecated.
        self.items.iter().any(|f| {
            f.chain.as_ref().map_or(false, |c| {
                c.value_is(&version.inner, |a| {
                    matches!(
                        a,
                        ItemStatus::Deprecation { .. }
                            | ItemStatus::NoChange {
                                previously_deprecated: true,
                                ..
                            }
                    )
                })
            })
        })
    }
}

// Kubernetes specific code generation
impl VersionedStruct {
    /// Generates the `kube::CustomResource` derive with the appropriate macro
    /// attributes.
    fn generate_kubernetes_cr_derive(&self, version: &VersionDefinition) -> Option<TokenStream> {
        if let Some(kubernetes_options) = &self.options.kubernetes_options {
            // Required arguments
            let group = &kubernetes_options.group;
            let version = version.inner.to_string();
            let kind = kubernetes_options
                .kind
                .as_ref()
                .map_or(self.idents.kubernetes.to_string(), |kind| kind.clone());

            // Optional arguments
            let namespaced = kubernetes_options
                .namespaced
                .then_some(quote! { , namespaced });
            let singular = kubernetes_options
                .singular
                .as_ref()
                .map(|s| quote! { , singular = #s });
            let plural = kubernetes_options
                .plural
                .as_ref()
                .map(|p| quote! { , plural = #p });

            return Some(quote! {
                #[derive(::kube::CustomResource)]
                #[kube(group = #group, version = #version, kind = #kind #singular #plural #namespaced)]
            });
        }

        None
    }

    /// Generates the `merge_crds` function call.
    fn generate_kubernetes_merge_crds(
        &self,
        kubernetes_definitions: Vec<KubernetesTokens>,
    ) -> TokenStream {
        let enum_ident = &self.idents.kubernetes;
        let enum_vis = &self.visibility;

        let mut enum_display_impl_matches = TokenStream::new();
        let mut enum_variant_idents = TokenStream::new();
        let mut merged_crd_fn_calls = TokenStream::new();

        for KubernetesTokens {
            merged_crd_fn_call,
            variant_display,
            enum_variant,
        } in kubernetes_definitions
        {
            merged_crd_fn_calls.extend(quote! {
                #merged_crd_fn_call,
            });

            enum_variant_idents.extend(quote! {
                #enum_variant,
            });

            enum_display_impl_matches.extend(quote! {
                #enum_ident::#enum_variant => f.write_str(#variant_display),
            });
        }

        quote! {
            #[automatically_derived]
            #enum_vis enum #enum_ident {
                #enum_variant_idents
            }

            #[automatically_derived]
            impl ::std::fmt::Display for #enum_ident {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::result::Result<(), ::std::fmt::Error> {
                    match self {
                        #enum_display_impl_matches
                    }
                }
            }

            #[automatically_derived]
            impl #enum_ident {
                /// Generates a merged CRD which contains all versions defined using the `#[versioned()]` macro.
                pub fn merged_crd(
                    stored_apiversion: Self
                ) -> ::std::result::Result<::k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition, ::kube::core::crd::MergeError> {
                    ::kube::core::crd::merge_crds(vec![#merged_crd_fn_calls], &stored_apiversion.to_string())
                }

                /// Generates and writes a merged CRD which contains all versions defined using the `#[versioned()]`
                /// macro to a file located at `path`.
                pub fn write_merged_crd<P>(path: P, stored_apiversion: Self, operator_version: &str) -> Result<(), ::stackable_versioned::Error>
                    where P: AsRef<::std::path::Path>
                {
                    use ::stackable_shared::yaml::{YamlSchema, SerializeOptions};

                    let merged_crd = Self::merged_crd(stored_apiversion).map_err(|err| ::stackable_versioned::Error::MergeCrd {
                        source: err,
                    })?;

                    YamlSchema::write_yaml_schema(
                        &merged_crd,
                        path,
                        operator_version,
                        SerializeOptions::default()
                    ).map_err(|err| ::stackable_versioned::Error::SerializeYaml {
                        source: err,
                    })
                }

                /// Generates and writes a merged CRD which contains all versions defined using the `#[versioned()]`
                /// macro to stdout.
                pub fn print_merged_crd(stored_apiversion: Self, operator_version: &str) -> Result<(), ::stackable_versioned::Error> {
                    use ::stackable_shared::yaml::{YamlSchema, SerializeOptions};

                    let merged_crd = Self::merged_crd(stored_apiversion).map_err(|err| ::stackable_versioned::Error::MergeCrd {
                        source: err,
                    })?;

                    YamlSchema::print_yaml_schema(
                        &merged_crd,
                        operator_version,
                        SerializeOptions::default()
                    ).map_err(|err| ::stackable_versioned::Error::SerializeYaml {
                        source: err,
                    })
                }
            }
        }
    }

    /// Generates the inner `crd()` functions calls which get used in the
    /// `merge_crds` function.
    fn generate_kubernetes_crd_fn_call(&self, version: &VersionDefinition) -> TokenStream {
        let struct_ident = &self.idents.kubernetes;
        let version_ident = &version.ident;
        let path: syn::Path = parse_quote!(#version_ident::#struct_ident);

        quote! {
            <#path as ::kube::CustomResourceExt>::crd()
        }
    }
}
