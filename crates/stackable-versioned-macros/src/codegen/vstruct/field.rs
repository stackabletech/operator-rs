use std::ops::{Deref, DerefMut};

use darling::FromField;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Field, Ident};

use crate::{
    attrs::{
        common::{ContainerAttributes, ItemAttributes},
        field::FieldAttributes,
    },
    codegen::common::{
        remove_deprecated_field_prefix, Attributes, ContainerVersion, Item, ItemStatus, Named,
        VersionedItem,
    },
};

/// A versioned field, which contains contains common [`Field`] data and a chain
/// of actions.
///
/// The chain of action maps versions to an action and the appropriate field
/// name. Additionally, the [`Field`] data can be used to forward attributes,
/// generate documentation, etc.
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
    fn common_attrs_owned(self) -> ItemAttributes {
        self.common
    }

    fn common_attrs(&self) -> &ItemAttributes {
        &self.common
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
        container_attributes: &ContainerAttributes,
    ) -> syn::Result<Self> {
        let item = VersionedItem::<_, FieldAttributes>::new(field, container_attributes)?;
        Ok(Self(item))
    }

    /// Generates tokens to be used in a container definition.
    pub(crate) fn generate_for_container(
        &self,
        container_version: &ContainerVersion,
    ) -> Option<TokenStream> {
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
                    ItemStatus::Added { ident, .. } => Some(quote! {
                        pub #ident: #field_type,
                    }),
                    ItemStatus::Renamed { to, .. } => Some(quote! {
                        pub #to: #field_type,
                    }),
                    ItemStatus::Deprecated {
                        ident: field_ident,
                        note,
                        ..
                    } => Some(quote! {
                        #[deprecated = #note]
                        pub #field_ident: #field_type,
                    }),
                    ItemStatus::NotPresent => None,
                    ItemStatus::NoChange(field_ident) => Some(quote! {
                        pub #field_ident: #field_type,
                    }),
                }
            }
            None => {
                // If there is no chain of field actions, the field is not
                // versioned and code generation is straight forward.
                // Unversioned fields are always included in versioned structs.
                let field_ident = &self.inner.ident;
                let field_type = &self.inner.ty;

                Some(quote! {
                    pub #field_ident: #field_type,
                })
            }
        }
    }

    /// Generates tokens to be used in a [`From`] implementation.
    pub(crate) fn generate_for_from_impl(
        &self,
        version: &ContainerVersion,
        next_version: &ContainerVersion,
        from_ident: &Ident,
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
                    (_, ItemStatus::Added { ident, default_fn }) => quote! {
                        #ident: #default_fn(),
                    },
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
