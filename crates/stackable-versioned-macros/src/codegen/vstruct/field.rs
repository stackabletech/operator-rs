use std::ops::{Deref, DerefMut};

use darling::{util::IdentString, FromField};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Field, Ident};

use crate::{
    attrs::{
        common::{ItemAttributes, StandaloneContainerAttributes},
        field::FieldAttributes,
    },
    codegen::common::{
        remove_deprecated_field_prefix, Attributes, InnerItem, Item, ItemStatus, Named,
        VersionDefinition, VersionedItem,
    },
};

/// A versioned field, which contains common [`Field`] data and a chain of
/// actions.
///
/// The chain of actions maps versions to an action and the appropriate field
/// name.
///
/// Additionally, the [`Field`] data can be used to forward attributes, generate
/// documentation, etc.
#[derive(Debug)]
pub(crate) struct VersionedField(VersionedItem<Field, FieldAttributes>);

impl Deref for VersionedField {
    type Target = VersionedItem<Field, FieldAttributes>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for VersionedField {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl TryFrom<&Field> for FieldAttributes {
    type Error = darling::Error;

    fn try_from(field: &Field) -> Result<Self, Self::Error> {
        Self::from_field(field)
    }
}

impl Attributes for FieldAttributes {
    fn common_attributes_owned(self) -> ItemAttributes {
        self.common
    }

    fn common_attributes(&self) -> &ItemAttributes {
        &self.common
    }

    fn original_attributes(&self) -> &Vec<syn::Attribute> {
        &self.attrs
    }
}

impl InnerItem for Field {
    fn ty(&self) -> syn::Type {
        self.ty.clone()
    }
}

impl Named for Field {
    fn cleaned_ident(&self) -> Ident {
        let ident = self.ident();
        remove_deprecated_field_prefix(ident)
    }

    fn ident(&self) -> &Ident {
        self.ident
            .as_ref()
            .expect("internal error: field must have an ident")
    }
}

impl VersionedField {
    /// Creates a new versioned field.
    ///
    /// Internally this calls [`VersionedItem::new`] to handle most of the
    /// common creation code.
    pub(crate) fn new(
        field: Field,
        container_attributes: &StandaloneContainerAttributes,
    ) -> syn::Result<Self> {
        let item = VersionedItem::<_, FieldAttributes>::new(field, container_attributes)?;
        Ok(Self(item))
    }

    /// Generates tokens to be used in a container definition.
    pub(crate) fn generate_for_container(
        &self,
        container_version: &VersionDefinition,
    ) -> Option<TokenStream> {
        let original_attributes = &self.original_attributes;

        match &self.chain {
            Some(chain) => {
                // Check if the provided container version is present in the map
                // of actions. If it is, some action occurred in exactly that
                // version and thus code is generated for that field based on
                // the type of action.
                // If not, the provided version has no action attached to it.
                // The code generation then depends on the relation to other
                // versions (with actions).

                let field_type = &self.inner.ty;

                // NOTE (@Techassi): https://rust-lang.github.io/rust-clippy/master/index.html#/expect_fun_call
                match chain.get(&container_version.inner).unwrap_or_else(|| {
                    panic!(
                        "internal error: chain must contain container version {}",
                        container_version.inner
                    )
                }) {
                    ItemStatus::Addition { ident, ty, .. } => Some(quote! {
                        #(#original_attributes)*
                        pub #ident: #ty,
                    }),
                    ItemStatus::Change {
                        to_ident, to_type, ..
                    } => Some(quote! {
                        #(#original_attributes)*
                        pub #to_ident: #to_type,
                    }),
                    ItemStatus::Deprecation {
                        ident: field_ident,
                        note,
                        ..
                    } => {
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
                            pub #field_ident: #field_type,
                        })
                    }
                    ItemStatus::NotPresent => None,
                    ItemStatus::NoChange {
                        previously_deprecated,
                        ident,
                        ty,
                        ..
                    } => {
                        // TODO (@Techassi): Also carry along the deprecation
                        // note.
                        let deprecated_attr = previously_deprecated.then(|| quote! {#[deprecated]});

                        Some(quote! {
                            #(#original_attributes)*
                            #deprecated_attr
                            pub #ident: #ty,
                        })
                    }
                }
            }
            None => {
                // If there is no chain of field actions, the field is not
                // versioned and therefore included in all versions.
                let field_ident = &self.inner.ident;
                let field_type = &self.inner.ty;

                Some(quote! {
                    #(#original_attributes)*
                    pub #field_ident: #field_type,
                })
            }
        }
    }

    /// Generates tokens to be used in a [`From`] implementation.
    pub(crate) fn generate_for_from_impl(
        &self,
        version: &VersionDefinition,
        next_version: &VersionDefinition,
        from_ident: &IdentString,
    ) -> TokenStream {
        match &self.chain {
            Some(chain) => {
                match (
                    chain
                        .get(&version.inner)
                        .expect("internal error: chain must contain container version"),
                    chain
                        .get(&next_version.inner)
                        .expect("internal error: chain must contain container version"),
                ) {
                    (
                        _,
                        ItemStatus::Addition {
                            ident, default_fn, ..
                        },
                    ) => quote! {
                        #ident: #default_fn(),
                    },
                    (
                        _,
                        ItemStatus::Change {
                            from_ident: old_field_ident,
                            to_ident,
                            from_type,
                            to_type,
                        },
                    ) => {
                        if from_type == to_type {
                            quote! {
                                #to_ident: #from_ident.#old_field_ident,
                            }
                        } else {
                            quote! {
                                #to_ident: #from_ident.#old_field_ident.into(),
                            }
                        }
                    }
                    (old, next) => {
                        let old_field_ident = old
                            .get_ident()
                            .expect("internal error: old field must have a name");

                        let next_field_ident = next
                            .get_ident()
                            .expect("internal error: new field must have a name");

                        quote! {
                            #next_field_ident: #from_ident.#old_field_ident,
                        }
                    }
                }
            }
            None => {
                let field_ident = &self.inner.ident;
                quote! {
                    #field_ident: #from_ident.#field_ident,
                }
            }
        }
    }
}
