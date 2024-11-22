use std::ops::Not;

use darling::util::IdentString;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{token::Pub, Ident, Visibility};

use crate::codegen::{container::Container, VersionDefinition};

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

        // If the 'preserve_module' flag is provided by the user, we need to change the visibility
        // of version modules (eg. 'v1alpha1') to be public, so that they are accessible inside the
        // preserved (wrapping) module. Otherwise, we can inherit the visibility from the module
        // which will be erased.
        let version_module_vis = if self.preserve_module {
            &Visibility::Public(Pub::default())
        } else {
            &self.vis
        };

        let mut tokens = TokenStream::new();

        let module_ident = &self.ident;
        let module_vis = &self.vis;

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

        if self.preserve_module {
            quote! {
                #[automatically_derived]
                #module_vis mod #module_ident {
                    #tokens
                }
            }
        } else {
            tokens
        }
    }
}
