use std::ops::Deref;

use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_quote, DataStruct, Error, Ident};

use crate::{
    attrs::common::ContainerAttributes,
    codegen::{
        common::{
            Container, ContainerInput, ContainerVersion, Item, VersionExt, VersionedContainer,
        },
        vstruct::field::VersionedField,
    },
};

pub(crate) mod field;

type GenerateVersionReturn = (TokenStream, Option<(TokenStream, (Ident, String))>);

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

impl Container<DataStruct, VersionedField> for VersionedStruct {
    fn new(
        input: ContainerInput,
        data: DataStruct,
        attributes: ContainerAttributes,
    ) -> syn::Result<Self> {
        let ident = &input.ident;

        // Convert the raw version attributes into a container version.
        let versions: Vec<_> = (&attributes).into();

        // Extract the field attributes for every field from the raw token
        // stream and also validate that each field action version uses a
        // version declared by the container attribute.
        let mut items = Vec::new();

        for field in data.fields {
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
        if attributes.kubernetes_attrs.is_some() && !ident.to_string().ends_with("Spec") {
            return Err(Error::new(
                ident.span(),
                "struct name needs to include the `Spec` suffix if Kubernetes features are enabled via `#[versioned(k8s())]`"
            ));
        }

        Ok(Self(VersionedContainer::new(
            input, attributes, versions, items,
        )))
    }

    fn generate_tokens(&self) -> TokenStream {
        let mut tokens = TokenStream::new();

        let mut enum_variants = Vec::new();
        let mut crd_fn_calls = Vec::new();

        let mut versions = self.versions.iter().peekable();

        while let Some(version) = versions.next() {
            let (container_definition, merged_crd) =
                self.generate_version(version, versions.peek().copied());

            if let Some((crd_fn_call, enum_variant)) = merged_crd {
                enum_variants.push(enum_variant);
                crd_fn_calls.push(crd_fn_call);
            }

            tokens.extend(container_definition);
        }

        if !crd_fn_calls.is_empty() {
            tokens.extend(self.generate_kubernetes_merge_crds(crd_fn_calls, enum_variants));
        }

        tokens
    }
}

impl VersionedStruct {
    /// Generates all tokens for a single instance of a versioned struct.
    fn generate_version(
        &self,
        version: &ContainerVersion,
        next_version: Option<&ContainerVersion>,
    ) -> GenerateVersionReturn {
        let mut token_stream = TokenStream::new();

        let original_attributes = &self.original_attributes;
        let struct_name = &self.idents.original;
        let visibility = &self.visibility;

        // Generate fields of the struct for `version`.
        let fields = self.generate_struct_fields(version);

        // TODO (@Techassi): Make the generation of the module optional to
        // enable the attribute macro to be applied to a module which
        // generates versioned versions of all contained containers.

        let version_ident = &version.ident;

        let deprecated_note = format!("Version {version} is deprecated", version = version_ident);
        let deprecated_attr = version
            .deprecated
            .then_some(quote! {#[deprecated = #deprecated_note]});

        // Generate doc comments for the container (struct)
        let version_specific_docs = self.generate_struct_docs(version);

        // Generate K8s specific code
        let (kubernetes_cr_derive, merged_crd) = match &self.options.kubernetes_options {
            Some(options) => {
                // Generate the CustomResource derive macro with the appropriate
                // attributes supplied using #[kube()].
                let cr_derive = self.generate_kubernetes_cr_derive(version);

                // Generate merged_crd specific code when not opted out.
                let merged_crd = if !options.skip_merged_crd {
                    let crd_fn_call = self.generate_kubernetes_crd_fn_call(version);
                    let enum_variant = version.inner.as_variant_ident();
                    let enum_display = version.inner.to_string();

                    Some((crd_fn_call, (enum_variant, enum_display)))
                } else {
                    None
                };

                (Some(cr_derive), merged_crd)
            }
            None => (None, None),
        };

        // Generate tokens for the module and the contained struct
        token_stream.extend(quote! {
            #[automatically_derived]
            #deprecated_attr
            #visibility mod #version_ident {
                use super::*;

                #version_specific_docs
                #(#original_attributes)*
                #kubernetes_cr_derive
                pub struct #struct_name {
                    #fields
                }
            }
        });

        // Generate the From impl between this `version` and the next one.
        if !self.options.skip_from && !version.skip_from {
            token_stream.extend(self.generate_from_impl(version, next_version));
        }

        (token_stream, merged_crd)
    }

    /// Generates version specific doc comments for the struct.
    fn generate_struct_docs(&self, version: &ContainerVersion) -> TokenStream {
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
    fn generate_struct_fields(&self, version: &ContainerVersion) -> TokenStream {
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
        version: &ContainerVersion,
        next_version: Option<&ContainerVersion>,
    ) -> Option<TokenStream> {
        if let Some(next_version) = next_version {
            let next_module_name = &next_version.ident;
            let module_name = &version.ident;

            let struct_ident = &self.idents.original;
            let from_ident = &self.idents.from;

            let fields = self.generate_from_fields(version, next_version, from_ident);

            // TODO (@Techassi): Be a little bit more clever about when to include
            // the #[allow(deprecated)] attribute.
            return Some(quote! {
                #[automatically_derived]
                #[allow(deprecated)]
                impl From<#module_name::#struct_ident> for #next_module_name::#struct_ident {
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
        version: &ContainerVersion,
        next_version: &ContainerVersion,
        from_ident: &Ident,
    ) -> TokenStream {
        let mut token_stream = TokenStream::new();

        for item in &self.items {
            token_stream.extend(item.generate_for_from_impl(version, next_version, from_ident))
        }

        token_stream
    }
}

// Kubernetes specific code generation
impl VersionedStruct {
    /// Generates the `kube::CustomResource` derive with the appropriate macro
    /// attributes.
    fn generate_kubernetes_cr_derive(&self, version: &ContainerVersion) -> Option<TokenStream> {
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
        crd_fn_calls: Vec<TokenStream>,
        enum_variants: Vec<(Ident, String)>,
    ) -> TokenStream {
        let enum_ident = &self.idents.kubernetes;
        let enum_vis = &self.visibility;

        let mut enum_display_impl_matches = TokenStream::new();
        let mut enum_variant_idents = TokenStream::new();

        for (enum_variant_ident, enum_variant_display) in enum_variants {
            enum_variant_idents.extend(quote! {#enum_variant_ident,});
            enum_display_impl_matches.extend(quote! {
                #enum_ident::#enum_variant_ident => f.write_str(#enum_variant_display),
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
                /// Generates a merged CRD which contains all versions defined using the
                /// `#[versioned()]` macro.
                pub fn merged_crd(
                    stored_apiversion: Self
                ) -> ::std::result::Result<::k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition, ::kube::core::crd::MergeError> {
                    ::kube::core::crd::merge_crds(vec![#(#crd_fn_calls),*], &stored_apiversion.to_string())
                }
            }
        }
    }

    /// Generates the inner `crd()` functions calls which get used in the
    /// `merge_crds` function.
    fn generate_kubernetes_crd_fn_call(&self, version: &ContainerVersion) -> TokenStream {
        let struct_ident = &self.idents.kubernetes;
        let version_ident = &version.ident;
        let path: syn::Path = parse_quote!(#version_ident::#struct_ident);

        quote! {
            <#path as ::kube::CustomResourceExt>::crd()
        }
    }
}
