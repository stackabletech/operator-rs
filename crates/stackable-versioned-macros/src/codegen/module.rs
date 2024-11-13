use darling::util::IdentString;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{token::Pub, Ident, Visibility};

use crate::codegen::{container::Container, VersionDefinition};

pub(crate) struct ModuleInput {
    pub(crate) vis: Visibility,
    pub(crate) ident: Ident,
}

pub(crate) struct Module {
    versions: Vec<VersionDefinition>,
    containers: Vec<Container>,
    preserve_module: bool,
    ident: IdentString,
    vis: Visibility,
}

impl Module {
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

    pub(crate) fn generate_tokens(&self) -> TokenStream {
        if self.containers.is_empty() {
            return quote! {};
        }

        // TODO (@Techassi): Leave comment explaining this
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
                    true,
                ));
            }

            tokens.extend(quote! {
                #version_module_vis mod #version_ident {
                    use super::*;

                    #container_definitions
                }

                #from_impls
            });
        }

        if self.preserve_module {
            quote! {
                #module_vis mod #module_ident {
                    #tokens
                }
            }
        } else {
            tokens
        }
    }
}
