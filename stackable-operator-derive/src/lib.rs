use darling::{ast::Data, FromDeriveInput, FromField, FromMeta};
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{parse_macro_input, parse_quote, Generics, Path, WherePredicate};

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
    generics: Generics,
    data: Data<(), MergeField>,
    #[darling(default)]
    path_overrides: PathOverrides,
    #[darling(default)]
    bounds: Option<Vec<WherePredicate>>,
}

#[derive(FromField)]
struct MergeField {
    ident: Option<Ident>,
}

#[proc_macro_derive(Merge, attributes(merge))]
pub fn derive_merge(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let MergeInput {
        ident,
        mut generics,
        data,
        path_overrides: PathOverrides { merge: merge_mod },
        bounds,
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

    if let Some(bounds) = bounds {
        let where_clause = generics.make_where_clause();
        where_clause.predicates.extend(bounds);
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    quote! {
        impl #impl_generics #merge_mod::Merge for #ident #ty_generics #where_clause {
            fn merge(&mut self, defaults: &Self) {
                #merge_fields
            }
        }
    }
    .into()
}
