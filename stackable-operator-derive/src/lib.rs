use darling::{ast::Data, FromDeriveInput, FromField, FromMeta};
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{parse_macro_input, parse_quote, Path};

#[derive(FromMeta)]
struct PathOverrides {
    #[darling(default = "PathOverrides::default_merge")]
    merge: Path,
}
impl Default for PathOverrides {
    fn default() -> Self {
        Self {
            merge: Self::default_merge(),
        }
    }
}
impl PathOverrides {
    fn default_merge() -> Path {
        parse_quote!(::stackable_operator::config::merge)
    }
}

#[derive(FromDeriveInput)]
#[darling(attributes(merge))]
struct MergeInput {
    ident: Ident,
    data: Data<(), MergeField>,
    #[darling(default)]
    path_overrides: PathOverrides,
}

#[derive(FromField)]
struct MergeField {
    ident: Option<Ident>,
}

#[proc_macro_derive(Merge, attributes(merge))]
pub fn derive_merge(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let MergeInput {
        ident,
        data,
        path_overrides: PathOverrides { merge: merge_mod },
    } = match MergeInput::from_derive_input(&parse_macro_input!(input)) {
        Ok(input) => input,
        Err(err) => return err.write_errors().into(),
    };

    let fields = data.take_struct().unwrap().fields;
    let merge_fields = fields
        .into_iter()
        .enumerate()
        .map(|(field_index, field)| {
            let field_ident = if let Some(ident) = field.ident {
                quote! {#ident}
            } else {
                quote! {#field_index}
            };
            quote! {
                #merge_mod::Merge::merge(&mut self.#field_ident, &defaults.#field_ident);
            }
        })
        .collect::<TokenStream>();

    quote! {
        impl #merge_mod::Merge for #ident {
            fn merge(&mut self, defaults: &Self) {
                #merge_fields
            }
        }
    }
    .into()
}
