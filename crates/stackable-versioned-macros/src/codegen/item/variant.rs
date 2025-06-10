use std::collections::BTreeMap;

use darling::{FromVariant, Result, util::IdentString};
use k8s_version::Version;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Attribute, Fields, Type, TypeNever, Variant, token::Not};

use crate::{
    attrs::item::VariantAttributes,
    codegen::{
        ItemStatus, VersionDefinition,
        changes::{BTreeMapExt, ChangesetExt},
    },
    utils::VariantIdent,
};

pub struct VersionedVariant {
    pub original_attributes: Vec<Attribute>,
    pub changes: Option<BTreeMap<Version, ItemStatus>>,
    pub ident: VariantIdent,
    pub fields: Fields,
}

impl VersionedVariant {
    pub fn new(variant: Variant, versions: &[VersionDefinition]) -> Result<Self> {
        let variant_attributes = VariantAttributes::from_variant(&variant)?;
        variant_attributes.validate_versions(versions)?;

        let variant_ident = VariantIdent::from(variant.ident);

        // FIXME (@Techassi): The chain of changes currently doesn't track versioning of variant
        // date and as such, we just use the never type here. During codegen, we just re-emit the
        // variant data as is.
        let ty = Type::Never(TypeNever {
            bang_token: Not([Span::call_site()]),
        });
        let changes = variant_attributes.common.into_changeset(&variant_ident, ty);

        Ok(Self {
            original_attributes: variant_attributes.attrs,
            fields: variant.fields,
            ident: variant_ident,
            changes,
        })
    }

    pub fn insert_container_versions(&mut self, versions: &[VersionDefinition]) {
        if let Some(changes) = &mut self.changes {
            // FIXME (@Techassi): Support enum variants with data
            let ty = Type::Never(TypeNever {
                bang_token: Not([Span::call_site()]),
            });

            changes.insert_container_versions(versions, &ty);
        }
    }

    /// Generates tokens to be used in a container definition.
    pub fn generate_for_container(&self, version: &VersionDefinition) -> Option<TokenStream> {
        let original_attributes = &self.original_attributes;
        let fields = &self.fields;

        match &self.changes {
            // NOTE (@Techassi): `unwrap_or_else` used instead of `expect`.
            // See: https://rust-lang.github.io/rust-clippy/master/index.html#/expect_fun_call
            Some(changes) => match changes.get(&version.inner).unwrap_or_else(|| {
                panic!(
                    "internal error: chain must contain container version {}",
                    version.inner
                )
            }) {
                ItemStatus::Addition { ident, .. } => Some(quote! {
                    #(#original_attributes)*
                    #ident #fields,
                }),
                ItemStatus::Change { to_ident, .. } => Some(quote! {
                    #(#original_attributes)*
                    #to_ident #fields,
                }),
                ItemStatus::Deprecation { ident, note, .. } => {
                    // FIXME (@Techassi): Emitting the deprecated attribute
                    // should cary over even when the item status is
                    // 'NoChange'.
                    // TODO (@Techassi): Make the generation of deprecated
                    // items customizable. When a container is used as a K8s
                    // CRD, the item must continue to exist, even when
                    // deprecated. For other versioning use-cases, that
                    // might not be the case.
                    let deprecated_attr = if let Some(note) = note {
                        quote! {#[deprecated = #note]}
                    } else {
                        quote! {#[deprecated]}
                    };

                    Some(quote! {
                        #(#original_attributes)*
                        #deprecated_attr
                        #ident #fields,
                    })
                }
                ItemStatus::NoChange {
                    previously_deprecated,
                    ident,
                    ..
                } => {
                    // TODO (@Techassi): Also carry along the deprecation
                    // note.
                    let deprecated_attr = previously_deprecated.then(|| quote! {#[deprecated]});

                    Some(quote! {
                        #(#original_attributes)*
                        #deprecated_attr
                        #ident #fields,
                    })
                }
                ItemStatus::NotPresent => None,
            },
            None => {
                // If there is no chain of variant actions, the variant is not
                // versioned and code generation is straight forward.
                // Unversioned variants are always included in versioned enums.
                let ident = &self.ident;

                Some(quote! {
                    #(#original_attributes)*
                    #ident #fields,
                })
            }
        }
    }

    pub fn generate_for_upgrade_from_impl(
        &self,
        version: &VersionDefinition,
        next_version: &VersionDefinition,
        enum_ident: &IdentString,
    ) -> Option<TokenStream> {
        let variant_fields = self.fields_as_token_stream();

        match &self.changes {
            Some(changes) => {
                let next_change = changes.get_expect(&next_version.inner);
                let change = changes.get_expect(&version.inner);

                match (change, next_change) {
                    (_, ItemStatus::Addition { .. }) => None,
                    (old, next) => {
                        let next_version_ident = &next_version.idents.module;
                        let old_version_ident = &version.idents.module;

                        let next_variant_ident = next.get_ident();
                        let old_variant_ident = old.get_ident();

                        let old = quote! {
                            #old_version_ident::#enum_ident::#old_variant_ident #variant_fields
                        };
                        let next = quote! {
                            #next_version_ident::#enum_ident::#next_variant_ident #variant_fields
                        };

                        Some(quote! {
                            #old => #next,
                        })
                    }
                }
            }
            None => {
                let next_version_ident = &next_version.idents.module;
                let old_version_ident = &version.idents.module;
                let variant_ident = &*self.ident;

                let old = quote! {
                    #old_version_ident::#enum_ident::#variant_ident #variant_fields
                };
                let next = quote! {
                    #next_version_ident::#enum_ident::#variant_ident #variant_fields
                };

                Some(quote! {
                    #old => #next,
                })
            }
        }
    }

    pub fn generate_for_downgrade_from_impl(
        &self,
        version: &VersionDefinition,
        next_version: &VersionDefinition,
        enum_ident: &IdentString,
    ) -> Option<TokenStream> {
        let variant_fields = self.fields_as_token_stream();

        match &self.changes {
            Some(changes) => {
                let next_change = changes.get_expect(&next_version.inner);
                let change = changes.get_expect(&version.inner);

                match (change, next_change) {
                    (_, ItemStatus::Addition { .. }) => None,
                    (old, next) => {
                        let next_version_ident = &next_version.idents.module;
                        let old_version_ident = &version.idents.module;

                        let next_variant_ident = next.get_ident();
                        let old_variant_ident = old.get_ident();

                        let old = quote! {
                            #old_version_ident::#enum_ident::#old_variant_ident #variant_fields
                        };
                        let next = quote! {
                            #next_version_ident::#enum_ident::#next_variant_ident #variant_fields
                        };

                        Some(quote! {
                            #next => #old,
                        })
                    }
                }
            }
            None => {
                let next_version_ident = &next_version.idents.module;
                let old_version_ident = &version.idents.module;
                let variant_ident = &*self.ident;

                let old = quote! {
                    #old_version_ident::#enum_ident::#variant_ident #variant_fields
                };
                let next = quote! {
                    #next_version_ident::#enum_ident::#variant_ident #variant_fields
                };

                Some(quote! {
                    #next => #old,
                })
            }
        }
    }

    fn fields_as_token_stream(&self) -> Option<TokenStream> {
        match &self.fields {
            Fields::Named(fields_named) => {
                let fields: Vec<_> = fields_named
                    .named
                    .iter()
                    .map(|field| {
                        field
                            .ident
                            .as_ref()
                            .expect("named fields always have an ident")
                    })
                    .collect();

                Some(quote! { { #(#fields),* } })
            }
            Fields::Unnamed(fields_unnamed) => {
                let fields: Vec<_> = fields_unnamed
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(index, _)| format_ident!("__sv_{index}"))
                    .collect();

                Some(quote! { ( #(#fields),* ) })
            }
            Fields::Unit => None,
        }
    }
}
