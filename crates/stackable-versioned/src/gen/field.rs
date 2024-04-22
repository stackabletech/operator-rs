use proc_macro2::TokenStream;
use syn::Field;

use crate::{
    attrs::field::FieldActions,
    gen::{version::ContainerVersion, ToTokensExt},
};

pub(crate) struct VersionedField {
    // TODO (@Techassi): There can be multiple actions for one field (in
    // different versions). Add support for that here.
    pub(crate) _actions: FieldActions,
    pub(crate) _inner: Field,
}

impl ToTokensExt for VersionedField {
    fn to_tokens_for_version(&self, _version: &ContainerVersion) -> Option<TokenStream> {
        // match &self.actions {
        //     FieldAction::Added(added) => {
        //         // Skip generating the field, if the current generated
        //         // version appears before the field action version.
        //         if version.inner < *added.since {
        //             return None;
        //         }

        //         let field_name = &self.inner.ident;
        //         let field_type = &self.inner.ty;
        //         let doc = format!(" Added since `{}`.", *added.since);

        //         // TODO (@Techassi): Also forward other attributes
        //         let doc_attrs: Vec<&Attribute> = self
        //             .inner
        //             .attrs
        //             .iter()
        //             .filter(|a| a.path().is_ident("doc"))
        //             .collect();

        //         Some(quote! {
        //             #(#doc_attrs)*
        //             #[doc = ""]
        //             #[doc = #doc]
        //             pub #field_name: #field_type,
        //         })
        //     }
        //     FieldAction::Renamed(renamed) => {
        //         if version.inner < *renamed.since {
        //             // Use the original name for versions before the field action
        //             // version.
        //             let field_name = format_ident!("{}", *renamed.from);
        //             let field_type = &self.inner.ty;

        //             Some(quote! {
        //                 pub #field_name: #field_type,
        //             })
        //         } else {
        //             // If the version is greater than the field action version
        //             // (or equal), use the new field name.
        //             let field_name = &self.inner.ident;
        //             let field_type = &self.inner.ty;

        //             Some(quote! {
        //                 pub #field_name: #field_type,
        //             })
        //         }
        //     }
        //     FieldAction::Deprecated(deprecated) => {
        //         if version.inner < *deprecated.since {
        //             // Remove the deprecated_ prefix from the field name and use
        //             // it as the field name if the version is less than the
        //             // field action version.
        //             let field_name = format_ident!(
        //                 "{}",
        //                 &self
        //                     .inner
        //                     .ident
        //                     .as_ref()
        //                     .unwrap()
        //                     .to_string()
        //                     .replace("deprecated_", "")
        //             );
        //             let field_type = &self.inner.ty;

        //             Some(quote! {
        //                 pub #field_name: #field_type,
        //             })
        //         } else {
        //             // If the version is greater than the field action version
        //             // (or equal), use the prefixed field name.
        //             let field_name = &self.inner.ident;
        //             let field_type = &self.inner.ty;
        //             let deprecated_note = &*deprecated.note;

        //             Some(quote! {
        //                 #[deprecated = #deprecated_note]
        //                 pub #field_name: #field_type,
        //             })
        //         }
        //     }
        //     FieldAction::None => {
        //         // Generate fields without any attributes in every version.
        //         let field_name = &self.inner.ident;
        //         let field_type = &self.inner.ty;

        //         Some(quote! {
        //             pub #field_name: #field_type,
        //         })
        //     }
        // }
        None
        // todo!()
    }
}

impl VersionedField {
    pub(crate) fn new(field: Field, actions: FieldActions) -> Self {
        Self {
            _inner: field,
            _actions: actions,
        }
    }
}
