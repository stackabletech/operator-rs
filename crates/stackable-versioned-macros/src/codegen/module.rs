use std::{collections::HashMap, ops::Not};

use darling::{Error, Result, util::IdentString};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Item, ItemMod, ItemUse, Visibility, token::Pub};

use crate::{
    ModuleAttributes,
    codegen::{KubernetesTokens, VersionDefinition, container::Container},
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

    // Flags which influence generation
    preserve_module: bool,
    skip_from: bool,
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

        let preserve_module = module_attributes
            .common
            .options
            .preserve_module
            .is_present();

        let skip_from = module_attributes
            .common
            .options
            .skip
            .as_ref()
            .is_some_and(|opts| opts.from.is_present());

        let mut errors = Error::accumulator();
        let mut submodules = HashMap::new();
        let mut containers = Vec::new();

        for item in items {
            match item {
                Item::Enum(item_enum) => {
                    if let Some(container) =
                        errors.handle(Container::new_enum_nested(item_enum, &versions))
                    {
                        containers.push(container);
                    };
                }
                Item::Struct(item_struct) => {
                    if let Some(container) =
                        errors.handle(Container::new_struct_nested(item_struct, &versions))
                    {
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
                        "Item not allowed here. Please move it ouside of the versioned module",
                    )
                    .with_span(&disallowed_item),
                ),
            };
        }

        errors.finish_with(Self {
            ident: item_mod.ident.into(),
            vis: item_mod.vis,
            preserve_module,
            containers,
            submodules,
            skip_from,
            versions,
        })
    }

    /// Generates tokens for all versioned containers.
    pub fn generate_tokens(&self) -> TokenStream {
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

        let mut kubernetes_container_items: HashMap<Ident, KubernetesTokens> = HashMap::new();
        let mut versions = self.versions.iter().peekable();

        while let Some(version) = versions.next() {
            let next_version = versions.peek().copied();
            let mut container_definitions = TokenStream::new();
            let mut from_impls = TokenStream::new();

            let version_module_ident = &version.idents.module;

            for container in &self.containers {
                container_definitions.extend(container.generate_definition(version));

                if !self.skip_from {
                    from_impls.extend(container.generate_upgrade_from_impl(
                        version,
                        next_version,
                        self.preserve_module,
                    ));

                    from_impls.extend(container.generate_downgrade_from_impl(
                        version,
                        next_version,
                        self.preserve_module,
                    ));
                }

                // Generate Kubernetes specific code which is placed outside of the container
                // definition.
                if let Some(items) = container.generate_kubernetes_version_items(version) {
                    let entry = kubernetes_container_items
                        .entry(container.get_original_ident().clone())
                        .or_default();

                    entry.push(items);
                }
            }

            let submodule_imports = self.generate_submodule_imports(version);

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
                #version_module_vis mod #version_module_ident {
                    use super::*;

                    #submodule_imports

                    #container_definitions
                }

                #from_impls
            });
        }

        // Generate the final Kubernetes specific code for each container (which uses Kubernetes
        // specific features) which is appended to the end of container definitions.
        for container in &self.containers {
            if let Some(items) = kubernetes_container_items.get(container.get_original_ident()) {
                kubernetes_tokens.extend(container.generate_kubernetes_code(
                    &self.versions,
                    items,
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
