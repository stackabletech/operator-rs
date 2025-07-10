use std::{collections::HashMap, ops::Not};

use darling::{Error, Result, util::IdentString};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Item, ItemMod, ItemUse, Visibility, token::Pub};

use crate::{
    attrs::module::{
        CrateArguments, KubernetesConfigOptions, ModuleAttributes, ModuleOptions,
        ModuleSkipArguments,
    },
    codegen::{
        VersionDefinition,
        container::{Container, ContainerTokens, VersionedContainerTokens},
    },
};

/// A versioned module.
///
/// Versioned modules allow versioning multiple containers at once without introducing conflicting
/// version module definitions.
pub struct Module {
    versions: Vec<VersionDefinition>,

    // Recognized items of the module
    containers: Vec<Container>,
    submodules: HashMap<IdentString, Vec<ItemUse>>,

    ident: IdentString,
    vis: Visibility,

    crates: CrateArguments,
    options: ModuleOptions,
    skip: ModuleSkipArguments,
}

impl Module {
    /// Creates a new versioned module containing versioned containers.
    pub fn new(item_mod: ItemMod, module_attributes: ModuleAttributes) -> Result<Self> {
        let Some((_, items)) = item_mod.content else {
            return Err(
                Error::custom("the macro can only be used on module blocks").with_span(&item_mod)
            );
        };

        let versions: Vec<VersionDefinition> = (&module_attributes).into();

        let mut errors = Error::accumulator();
        let mut submodules = HashMap::new();
        let mut containers = Vec::new();

        for item in items {
            match item {
                Item::Enum(item_enum) => {
                    if let Some(container) =
                        errors.handle(Container::new_enum(item_enum, &versions))
                    {
                        containers.push(container);
                    };
                }
                Item::Struct(item_struct) => {
                    let experimental_conversion_tracking = module_attributes
                        .options
                        .kubernetes
                        .experimental_conversion_tracking
                        .is_present();

                    if let Some(container) = errors.handle(Container::new_struct(
                        item_struct,
                        &versions,
                        experimental_conversion_tracking,
                    )) {
                        containers.push(container);
                    }
                }
                Item::Mod(submodule) => {
                    if !versions
                        .iter()
                        .any(|v| v.idents.module.as_ident() == &submodule.ident)
                    {
                        errors.push(
                            Error::custom(
                                "submodules must use names which are defined as `version`s",
                            )
                            .with_span(&submodule),
                        );
                        continue;
                    }

                    match submodule.content {
                        Some((_, items)) => {
                            let use_statements: Vec<ItemUse> = items
                                .into_iter()
                                // We are only interested in use statements. Everything else is ignored.
                                .filter_map(|item| match item {
                                    Item::Use(item_use) => Some(item_use),
                                    item => {
                                        errors.push(
                                            Error::custom(
                                                "submodules must only define `use` statements",
                                            )
                                            .with_span(&item),
                                        );

                                        None
                                    }
                                })
                                .collect();

                            submodules.insert(submodule.ident.into(), use_statements);
                        }
                        None => errors.push(
                            Error::custom("submodules must be module blocks").with_span(&submodule),
                        ),
                    }
                }
                // NOTE (@NickLarsenNZ): We throw an error here so the developer isn't surprised when items they have
                // defined in the module are no longer accessible (because they are not re-emitted).
                disallowed_item => errors.push(
                    Error::custom(
                        "Item not allowed here. Please move it outside of the versioned module",
                    )
                    .with_span(&disallowed_item),
                ),
            };
        }

        errors.finish_with(Self {
            options: module_attributes.options,
            crates: module_attributes.crates,
            skip: module_attributes.skip,
            ident: item_mod.ident.into(),
            vis: item_mod.vis,
            containers,
            submodules,
            versions,
        })
    }

    /// Generates tokens for all versioned containers.
    pub fn generate_tokens(&self) -> TokenStream {
        if self.containers.is_empty() {
            return quote! {};
        }

        let preserve_module = self.options.common.preserve_module.is_present();

        let module_ident = &self.ident;
        let module_vis = &self.vis;

        // If the 'preserve_module' flag is provided by the user, we need to change the visibility
        // of version modules (eg. 'v1alpha1') to be public, so that they are accessible inside the
        // preserved (wrapping) module. Otherwise, we can inherit the visibility from the module
        // which will be erased.
        let version_module_vis = if preserve_module {
            &Visibility::Public(Pub::default())
        } else {
            &self.vis
        };

        let mut inner_and_between_tokens = HashMap::new();
        let mut outer_tokens = TokenStream::new();
        let mut tokens = TokenStream::new();

        let ctx = ModuleGenerationContext {
            kubernetes_options: &self.options.kubernetes,
            add_attributes: preserve_module,
            vis: version_module_vis,
            crates: &self.crates,
            skip: &self.skip,
        };

        for container in &self.containers {
            let ContainerTokens { versioned, outer } =
                container.generate_tokens(&self.versions, ctx);

            inner_and_between_tokens.insert(container.get_original_ident(), versioned);
            outer_tokens.extend(outer);
        }

        // Only add #[automatically_derived] here if the user doesn't want to preserve the
        // module.
        let automatically_derived = preserve_module
            .not()
            .then(|| quote! {#[automatically_derived]});

        for version in &self.versions {
            let mut inner_tokens = TokenStream::new();
            let mut between_tokens = TokenStream::new();

            for container in &self.containers {
                let versioned = inner_and_between_tokens
                    .get_mut(container.get_original_ident())
                    .unwrap();
                let VersionedContainerTokens { inner, between } =
                    versioned.remove(&version.inner).unwrap();

                inner_tokens.extend(inner);
                between_tokens.extend(between);
            }

            let version_module_ident = &version.idents.module;

            // Add the #[deprecated] attribute when the version is marked as deprecated.
            let deprecated_attribute = version
                .deprecated
                .as_ref()
                .map(|note| quote! { #[deprecated = #note] });

            let submodule_imports = self.generate_submodule_imports(version);

            tokens.extend(quote! {
                #automatically_derived
                #deprecated_attribute
                #version_module_vis mod #version_module_ident {
                    use super::*;

                    #submodule_imports
                    #inner_tokens
                }

                #between_tokens
            });
        }

        if preserve_module {
            quote! {
                #[automatically_derived]
                #module_vis mod #module_ident {
                    #tokens
                    #outer_tokens
                }
            }
        } else {
            quote! {
                #tokens
                #outer_tokens
            }
        }
    }

    /// Optionally generates imports (which can be re-exports) located in submodules for the
    /// specified `version`.
    fn generate_submodule_imports(&self, version: &VersionDefinition) -> Option<TokenStream> {
        self.submodules
            .get(&version.idents.module)
            .map(|use_statements| {
                quote! {
                    #(#use_statements)*
                }
            })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ModuleGenerationContext<'a> {
    pub kubernetes_options: &'a KubernetesConfigOptions,
    pub skip: &'a ModuleSkipArguments,
    pub crates: &'a CrateArguments,
    pub vis: &'a Visibility,

    pub add_attributes: bool,
}

impl ModuleGenerationContext<'_> {
    pub fn automatically_derived_attr(&self) -> Option<TokenStream> {
        self.add_attributes
            .not()
            .then(|| quote! { #[automatically_derived] })
    }
}
