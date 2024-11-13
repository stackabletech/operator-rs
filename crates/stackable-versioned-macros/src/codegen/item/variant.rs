use std::collections::BTreeMap;

use darling::{util::IdentString, FromVariant, Result};
use k8s_version::Version;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{token::Not, Attribute, Fields, Type, TypeNever, Variant};

use crate::{
    attrs::item::VariantAttributes,
    codegen::{
        changes::{BTreeMapExt, ChangesetExt},
        ItemStatus, VersionDefinition,
    },
    utils::VariantIdent,
};

pub(crate) struct VersionedVariant {
    pub(crate) original_attributes: Vec<Attribute>,
    pub(crate) changes: Option<BTreeMap<Version, ItemStatus>>,
    pub(crate) ident: VariantIdent,
    pub(crate) fields: Fields,
}

impl VersionedVariant {
    pub(crate) fn new(variant: Variant, versions: &[VersionDefinition]) -> Result<Self> {
        let variant_attributes = VariantAttributes::from_variant(&variant)?;
        variant_attributes.validate_versions(versions)?;

        let variant_ident = VariantIdent::from(variant.ident);

        // FIXME (@Techassi): As we currently don't support enum variants with
        // data, we just return the Never type as the code generation code for
        // enum variants won't use this type information.
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

    pub(crate) fn insert_container_versions(&mut self, versions: &[VersionDefinition]) {
        if let Some(changes) = &mut self.changes {
            // FIXME (@Techassi): Support enum variants with data
            let ty = Type::Never(TypeNever {
                bang_token: Not([Span::call_site()]),
            });

            changes.insert_container_versions(versions, &ty);
        }
    }

    /// Generates tokens to be used in a container definition.
    pub(crate) fn generate_for_container(
        &self,
        version: &VersionDefinition,
    ) -> Option<TokenStream> {
        let original_attributes = &self.original_attributes;
        let fields = &self.fields;

        match &self.changes {
            // NOTE (@Techassi): https://rust-lang.github.io/rust-clippy/master/index.html#/expect_fun_call
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

    pub(crate) fn generate_for_from_impl(
        &self,
        version: &VersionDefinition,
        next_version: &VersionDefinition,
        enum_ident: &IdentString,
    ) -> Option<TokenStream> {
        let variant_fields = match &self.fields {
            Fields::Named(fields_named) => {
                let fields: Vec<_> = fields_named
                    .named
                    .iter()
                    .map(|field| {
                        field
                            .ident
                            .as_ref()
                            .expect("named fields always have an ident")
                            .clone()
                    })
                    .collect();

                quote! { { #(#fields),* } }
            }
            Fields::Unnamed(fields_unnamed) => {
                let fields: Vec<_> = fields_unnamed
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(index, _)| format_ident!("__sv_{index}"))
                    .collect();

                quote! { ( #(#fields),* ) }
            }
            Fields::Unit => TokenStream::new(),
        };

        match &self.changes {
            Some(changes) => {
                let next_change = changes.get_expect(&next_version.inner);
                let change = changes.get_expect(&version.inner);

                match (change, next_change) {
                    (_, ItemStatus::Addition { .. }) => None,
                    (old, next) => {
                        let next_version_ident = &next_version.ident;
                        let old_version_ident = &version.ident;

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
                let next_version_ident = &next_version.ident;
                let old_version_ident = &version.ident;
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
}
