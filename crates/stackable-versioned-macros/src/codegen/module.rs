use std::{collections::HashMap, ops::Not};

use darling::util::IdentString;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{token::Pub, Ident, Visibility};

use crate::codegen::{container::Container, VersionDefinition};

pub(crate) type KubernetesItems = (Vec<TokenStream>, Vec<IdentString>, Vec<String>);

pub(crate) struct ModuleInput {
    pub(crate) vis: Visibility,
    pub(crate) ident: Ident,
}

/// A versioned module.
///
/// Versioned modules allow versioning multiple containers at once without introducing conflicting
/// version module definitions.
pub(crate) struct Module {
    versions: Vec<VersionDefinition>,
    containers: Vec<Container>,
    preserve_module: bool,
    ident: IdentString,
    vis: Visibility,
}

impl Module {
    /// Creates a new versioned module containing versioned containers.
    pub(crate) fn new(
        ModuleInput { ident, vis, .. }: ModuleInput,
        preserve_module: bool,
        versions: Vec<VersionDefinition>,
        containers: Vec<Container>,
    ) -> Self {
        Self {
            ident: ident.into(),
            preserve_module,
            containers,
            versions,
            vis,
        }
    }

    /// Generates tokens for all versioned containers.
    pub(crate) fn generate_tokens(&self) -> TokenStream {
        if self.containers.is_empty() {
            return quote! {};
        }

        let module_ident = &self.ident;
        let module_vis = &self.vis;

        // If the 'preserve_module' flag is provided by the user, we need to change the visibility
        // of version modules (eg. 'v1alpha1') to be public, so that they are accessible inside the
        // preserved (wrapping) module. Otherwise, we can inherit the visibility from the module
        // which will be erased.
        let version_module_vis = if self.preserve_module {
            &Visibility::Public(Pub::default())
        } else {
            &self.vis
        };

        let mut kubernetes_tokens = TokenStream::new();
        let mut tokens = TokenStream::new();

        let mut kubernetes_container_items: HashMap<Ident, KubernetesItems> = HashMap::new();
        let mut versions = self.versions.iter().peekable();

        while let Some(version) = versions.next() {
            let mut container_definitions = TokenStream::new();
            let mut from_impls = TokenStream::new();

            let version_ident = &version.ident;

            for container in &self.containers {
                container_definitions.extend(container.generate_definition(version));

                from_impls.extend(container.generate_from_impl(
                    version,
                    versions.peek().copied(),
                    self.preserve_module,
                ));

                // Generate Kubernetes specific code which is placed outside of the container
                // definition.
                if let Some((enum_variant_ident, enum_variant_string, fn_call)) =
                    container.generate_kubernetes_item(version)
                {
                    let entry = kubernetes_container_items
                        .entry(container.get_original_ident().clone())
                        .or_default();

                    entry.0.push(fn_call);
                    entry.1.push(enum_variant_ident);
                    entry.2.push(enum_variant_string);
                }
            }

            // Only add #[automatically_derived] here if the user doesn't want to preserve the
            // module.
            let automatically_derived = self
                .preserve_module
                .not()
                .then(|| quote! {#[automatically_derived]});

            // Add the #[deprecated] attribute when the version is marked as deprecated.
            let deprecated_attribute = version
                .deprecated
                .as_ref()
                .map(|note| quote! { #[deprecated = #note] });

            tokens.extend(quote! {
                #automatically_derived
                #deprecated_attribute
                #version_module_vis mod #version_ident {
                    use super::*;

                    #container_definitions
                }

                #from_impls
            });
        }

        // Generate the final Kubernetes specific code for each container (which uses Kubernetes
        // specific features) which is appended to the end of container definitions.
        for container in &self.containers {
            if let Some((
                kubernetes_merge_crds_fn_calls,
                kubernetes_enum_variant_idents,
                kubernetes_enum_variant_strings,
            )) = kubernetes_container_items.get(container.get_original_ident())
            {
                kubernetes_tokens.extend(container.generate_kubernetes_merge_crds(
                    kubernetes_enum_variant_idents,
                    kubernetes_enum_variant_strings,
                    kubernetes_merge_crds_fn_calls,
                    version_module_vis,
                    self.preserve_module,
                ));
            }
        }

        if self.preserve_module {
            quote! {
                #[automatically_derived]
                #module_vis mod #module_ident {
                    #tokens
                    #kubernetes_tokens
                }
            }
        } else {
            quote! {
                #tokens
                #kubernetes_tokens
            }
        }
    }
}
