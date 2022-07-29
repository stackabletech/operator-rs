use darling::{
    ast::{Data, Fields},
    FromDeriveInput, FromField, FromMeta, FromVariant,
};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote};
use syn::{parse_quote, DeriveInput, Generics, Index, Path, WherePredicate};

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
    data: Data<MergeVariant, MergeField>,
    #[darling(default)]
    path_overrides: PathOverrides,
    #[darling(default)]
    bounds: Option<Vec<WherePredicate>>,
}

#[derive(FromField)]
struct MergeField {
    ident: Option<Ident>,
}

#[derive(FromVariant)]
struct MergeVariant {
    ident: Ident,
    fields: Fields<MergeField>,
}

#[derive(Debug, PartialEq, Eq)]
enum InputType {
    Struct,
    Enum,
}

pub fn derive(input: DeriveInput) -> TokenStream {
    let MergeInput {
        ident,
        mut generics,
        data,
        path_overrides: PathOverrides { merge: merge_mod },
        bounds,
    } = match MergeInput::from_derive_input(&input) {
        Ok(input) => input,
        Err(err) => return err.write_errors(),
    };

    let (ty, variants) = match data {
        // Structs are almost single-variant enums, so we can reuse most of the same matching code for both cases
        Data::Struct(fields) => (
            InputType::Struct,
            vec![MergeVariant {
                ident: Ident::new("__placeholder", Span::call_site()),
                fields,
            }],
        ),
        Data::Enum(variants) => (InputType::Enum, variants),
    };
    let merge_variants = variants
        .into_iter()
        .map(
            |MergeVariant {
                 ident: variant_ident,
                 fields,
             }| {
                let constructor: Path = match ty {
                    InputType::Struct => parse_quote! {#ident},
                    InputType::Enum => parse_quote! {#ident::#variant_ident},
                };
                let self_ident = format_ident!("self");
                let defaults_ident = format_ident!("defaults");
                let field_idents = fields.iter().map(|f| f.ident.as_ref());
                let self_fields =
                    map_fields_to_prefixed_vars(&constructor, field_idents.clone(), &self_ident);
                let defaults_fields =
                    map_fields_to_prefixed_vars(&constructor, field_idents, &defaults_ident);
                let body = fields
                    .into_iter()
                    .enumerate()
                    .map(|(field_index, field)| {
                        let field_ident = field.ident.as_ref().ok_or(field_index);
                        let self_field = prefix_ident(field_ident, &self_ident);
                        let default_field = prefix_ident(field_ident, &defaults_ident);
                        quote! {
                            #merge_mod::Merge::merge(#self_field, #default_field);
                        }
                    })
                    .collect::<TokenStream>();

                let pattern = match ty {
                    InputType::Struct => quote! {(#self_fields, #defaults_fields)},
                    InputType::Enum => quote! {(Some(#self_fields), Some(#defaults_fields))},
                };
                quote! {
                    #pattern => {#body},
                }
            },
        )
        .collect::<TokenStream>();

    if let Some(bounds) = bounds {
        let where_clause = generics.make_where_clause();
        where_clause.predicates.extend(bounds);
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let ty_toks = match ty {
        InputType::Struct => quote! { #ident #ty_generics },
        // Enums need some way to indicate that we want to keep the same variant, in our case we use
        // Option::None to signal this
        InputType::Enum => quote! { Option<#ident #ty_generics> },
    };
    let fallback_variants = match ty {
        InputType::Struct => quote! {},
        InputType::Enum => quote! {
            // self is None => inherit everything from defaults
            (this @ None, defaults) => *this = <Self as ::std::clone::Clone>::clone(defaults),
            // self is Some but mismatches defaults, discard defaults
            (Some(_), _) => {}
        },
    };
    quote! {
        impl #impl_generics #merge_mod::Merge for #ty_toks #where_clause {
            fn merge(&mut self, defaults: &Self) {
                match (self, defaults) {
                    #merge_variants
                    #fallback_variants
                }
            }
        }
    }
}

fn map_fields_to_prefixed_vars<'a>(
    constructor: &Path,
    fields: impl IntoIterator<Item = Option<&'a Ident>>,
    prefix: &Ident,
) -> TokenStream {
    let fields = fields
        .into_iter()
        .enumerate()
        .map(|(index, field)| {
            let prefixed = prefix_ident(field.ok_or(index), prefix);
            if let Some(field) = field {
                quote! { #field: #prefixed, }
            } else {
                let index = Index::from(index);
                quote! { #index: #prefixed, }
            }
        })
        .collect::<TokenStream>();
    quote! { #constructor { #fields } }
}

fn prefix_ident(ident: Result<&Ident, usize>, prefix: &Ident) -> Ident {
    match ident {
        Ok(ident) => format_ident!("{prefix}_{ident}"),
        Err(index) => format_ident!("{prefix}_{index}"),
    }
}
