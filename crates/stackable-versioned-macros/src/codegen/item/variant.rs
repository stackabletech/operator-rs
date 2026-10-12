use std::collections::BTreeMap;

use darling::{FromVariant, Result, util::IdentString};
use k8s_version::Version;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    Attribute, Fields, FieldsNamed, FieldsUnnamed, Ident, Type, TypeNever, Variant, token::Not,
};

use crate::{
    attrs::item::VariantAttributes,
    codegen::{
        Direction, VersionDefinition,
        changes::{BTreeMapExt, ChangesetExt},
        item::{ItemStatus, generate_attributes},
        module::ModuleGenerationContext,
    },
    utils::ItemIdents,
};

pub struct VersionedVariant {
    pub original_attributes: Vec<Attribute>,
    pub previous_docs: BTreeMap<Version, Vec<String>>,
    pub changes: Option<BTreeMap<Version, ItemStatus>>,
    pub idents: VariantIdents,
    pub fields: Fields,
    pub nested: bool,
}

impl VersionedVariant {
    pub fn new(
        variant: Variant,
        versions: &[VersionDefinition],
        experimental_conversion_tracking: bool,
    ) -> Result<Self> {
        let variant_attributes = VariantAttributes::from_variant(&variant)?;
        variant_attributes.validate_versions(versions)?;
        variant_attributes.validate_nested_flag(experimental_conversion_tracking)?;

        let idents = VariantIdents::from(variant.ident);

        // FIXME (@Techassi): The chain of changes currently doesn't track versioning of variant
        // data and as such, we just use the never type here. During codegen, we just re-emit the
        // variant data as is.
        let ty = Type::Never(TypeNever {
            attrs: Vec::new(),
            bang_token: Not([Span::call_site()]),
        });
        let previous_docs = variant_attributes.common.previous_docs();
        let nested = variant_attributes.nested.is_present();
        let changes = variant_attributes.common.into_changeset(&idents, ty);

        Ok(Self {
            original_attributes: variant_attributes.attrs,
            previous_docs,
            fields: variant.fields,
            idents,
            changes,
            nested,
        })
    }

    pub fn insert_container_versions(&mut self, versions: &[VersionDefinition]) {
        if let Some(changes) = &mut self.changes {
            // FIXME (@Techassi): Support enum variants with data
            let ty = Type::Never(TypeNever {
                attrs: Vec::new(),
                bang_token: Not([Span::call_site()]),
            });

            changes.insert_container_versions(versions, &ty);
        }
    }

    /// Generates tokens to be used in a container definition.
    pub fn generate_for_container(&self, version: &VersionDefinition) -> Option<TokenStream> {
        let attributes = generate_attributes(
            &self.original_attributes,
            &self.previous_docs,
            &version.inner,
        );
        let fields = &self.fields;

        #[allow(clippy::single_match_else)]
        match &self.changes {
            // NOTE (@Techassi): `unwrap_or_else` used instead of `expect`.
            // See: https://rust-lang.github.io/rust-clippy/master/index.html#expect_fun_call
            // We could use expect here, but we would lose the version in the panic message. We need to allow
            // a lint in either case anyway.
            #[allow(clippy::panic)]
            Some(changes) => match changes.get(&version.inner).unwrap_or_else(|| {
                panic!(
                    "internal error: chain must contain container version {}",
                    version.inner
                )
            }) {
                ItemStatus::Addition { ident, .. } => Some(quote! {
                    #attributes
                    #ident #fields,
                }),
                ItemStatus::Change { to_ident, .. } => Some(quote! {
                    #attributes
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
                        #attributes
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
                        #attributes
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
                let ident = &self.idents.original;

                Some(quote! {
                    #attributes
                    #ident #fields,
                })
            }
        }
    }

    pub fn generate_for_from_impl(
        &self,
        direction: Direction,
        version: &VersionDefinition,
        next_version: &VersionDefinition,
        enum_ident: &IdentString,
        mod_gen_ctx: ModuleGenerationContext<'_>,
    ) -> Option<TokenStream> {
        let from_fields = self.generate_from_fields();
        let for_fields = self.generate_for_fields(mod_gen_ctx);

        #[allow(clippy::single_match_else)]
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

                        match direction {
                            Direction::Upgrade => Some(quote! {
                                #old_version_ident::#enum_ident::#old_variant_ident #from_fields
                                    => #next_version_ident::#enum_ident::#next_variant_ident #for_fields,
                            }),
                            Direction::Downgrade => Some(quote! {
                                #next_version_ident::#enum_ident::#next_variant_ident #from_fields
                                    => #old_version_ident::#enum_ident::#old_variant_ident #for_fields,
                            }),
                        }
                    }
                }
            }
            None => {
                let next_version_ident = &next_version.idents.module;
                let old_version_ident = &version.idents.module;
                let variant_ident = &self.idents.original;

                match direction {
                    Direction::Upgrade => Some(quote! {
                        #old_version_ident::#enum_ident::#variant_ident #from_fields
                            => #next_version_ident::#enum_ident::#variant_ident #for_fields,
                    }),
                    Direction::Downgrade => Some(quote! {
                        #next_version_ident::#enum_ident::#variant_ident #from_fields
                            => #old_version_ident::#enum_ident::#variant_ident #for_fields,
                    }),
                }
            }
        }
    }

    fn generate_for_fields(&self, mod_gen_ctx: ModuleGenerationContext<'_>) -> Option<TokenStream> {
        match &self.fields {
            Fields::Named(fields_named) => {
                let fields = Self::named_field_idents(fields_named);
                let conversions = fields.iter().map(|field| {
                    self.generate_conversion_function(Some(&field.to_string()), mod_gen_ctx)
                });

                Some(quote! { { #(#fields: #fields.#conversions,)* } })
            }
            Fields::Unnamed(fields_unnamed) => {
                let fields = Self::unnamed_field_ident(fields_unnamed);

                // Newtype variants (which are the most common variants with data) don't need an
                // additional path segment, as the variant only contains a single field.
                let conversions = (0..fields.len()).map(|index| {
                    let child = (fields.len() > 1).then(|| index.to_string());
                    self.generate_conversion_function(child.as_deref(), mod_gen_ctx)
                });

                Some(quote! { ( #(#fields.#conversions),* ) })
            }
            Fields::Unit => None,
        }
    }

    /// Generates the conversion function for a single field of the variant data.
    ///
    /// The data of variants marked as nested is converted with support for tracking. The path
    /// passed down consists of the variant name and the provided `child`, if any.
    fn generate_conversion_function(
        &self,
        child: Option<&str>,
        mod_gen_ctx: ModuleGenerationContext<'_>,
    ) -> TokenStream {
        if !self.nested {
            return quote! { into() };
        }

        let versioned_path = &*mod_gen_ctx.crates.versioned;
        let variant = &self.idents.original;
        let child_string = match child {
            Some(child) => format!("{variant}.{child}"),
            None => variant.to_string(),
        };

        quote! { tracking_into(status, &#versioned_path::jthong_path(parent, #child_string)) }
    }

    fn generate_from_fields(&self) -> Option<TokenStream> {
        match &self.fields {
            Fields::Named(fields_named) => {
                let fields = Self::named_field_idents(fields_named);
                Some(quote! { { #(#fields,)* } })
            }
            Fields::Unnamed(fields_unnamed) => {
                let fields = Self::unnamed_field_ident(fields_unnamed);
                Some(quote! { ( #(#fields),* ) })
            }
            Fields::Unit => None,
        }
    }

    fn named_field_idents(fields_named: &FieldsNamed) -> Vec<&Ident> {
        fields_named
            .named
            .iter()
            .map(|field| {
                field
                    .ident
                    .as_ref()
                    .expect("named fields always have an ident")
            })
            .collect()
    }

    fn unnamed_field_ident(fields_unnamed: &FieldsUnnamed) -> Vec<Ident> {
        fields_unnamed
            .unnamed
            .iter()
            .enumerate()
            .map(|(index, _)| format_ident!("__sv_{index}"))
            .collect()
    }
}

/// A collection of variant idents used for different purposes.
#[derive(Debug)]
pub struct VariantIdents {
    /// The original ident.
    pub original: IdentString,

    /// The cleaned ident, with the deprecation prefix removed.
    pub cleaned: IdentString,
}

impl ItemIdents for VariantIdents {
    const DEPRECATION_PREFIX: &str = "Deprecated";

    fn cleaned(&self) -> &IdentString {
        &self.cleaned
    }

    fn original(&self) -> &IdentString {
        &self.original
    }
}

impl From<Ident> for VariantIdents {
    fn from(ident: Ident) -> Self {
        let original = IdentString::new(ident);
        let cleaned = original
            .clone()
            .map(|s| s.trim_start_matches(Self::DEPRECATION_PREFIX).to_owned());

        Self { original, cleaned }
    }
}
