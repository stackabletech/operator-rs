use darling::{ast::Data, FromDeriveInput, FromField, FromMeta, FromVariant};
use proc_macro2::{Ident, TokenStream, TokenTree};
use quote::{format_ident, quote};
use syn::{
    parse_quote, Attribute, DeriveInput, Expr, ExprLit, Generics, Lit, Meta, MetaList,
    MetaNameValue, Path, Type, Visibility, WherePredicate,
};

#[derive(FromMeta)]
struct PathOverrides {
    #[darling(default = "PathOverrides::default_fragment")]
    fragment: Path,
    #[darling(default = "PathOverrides::default_result")]
    result: Path,
}
impl std::default::Default for PathOverrides {
    fn default() -> Self {
        Self {
            fragment: Self::default_fragment(),
            result: Self::default_result(),
        }
    }
}
impl PathOverrides {
    fn default_fragment() -> Path {
        parse_quote!(::stackable_operator::config::fragment)
    }

    fn default_result() -> Path {
        parse_quote!(::core::result)
    }
}

#[derive(FromDeriveInput)]
#[darling(attributes(fragment), forward_attrs(fragment_attrs, doc))]
pub struct FragmentInput {
    ident: Ident,
    generics: Generics,
    data: Data<FragmentVariant, FragmentField>,
    attrs: Vec<Attribute>,
    #[darling(default)]
    path_overrides: PathOverrides,
    #[darling(default)]
    bound: Option<Vec<WherePredicate>>,
}

fn split_by_comma(tokens: TokenStream) -> Vec<TokenStream> {
    let mut iter = tokens.into_iter().fuse().peekable();
    let mut groups = Vec::new();
    while iter.peek().is_some() {
        groups.push(
            iter.by_ref()
                .take_while(
                    |token| !matches!(token, TokenTree::Punct(punct) if punct.as_char() == ','),
                )
                .collect(),
        );
    }
    groups
}

enum ExtractAttrsError {
    InvalidAttrForm,
    InvalidDocAttrForm,
}
impl ExtractAttrsError {
    fn into_compile_error(self) -> TokenStream {
        match self {
            Self::InvalidAttrForm => quote! {
                compile_error!("`#[fragment_attrs]` only takes list-form parameters");
            },
            Self::InvalidDocAttrForm => quote! {
                compile_error!("`#[doc]` only supports doc comments with string literals");
            },
        }
    }
}

fn extract_forwarded_attrs(attrs: &[Attribute]) -> Result<TokenStream, ExtractAttrsError> {
    attrs
        .iter()
        .filter_map(|attr| {
            if attr.path().is_ident("fragment_attrs") {
                match &attr.meta {
                    Meta::List(MetaList { tokens, .. }) => {
                        Some(split_by_comma(tokens.clone()).into_iter().map(Ok).collect())
                    }
                    _ => Some(vec![Err(ExtractAttrsError::InvalidAttrForm)]),
                }
            } else if attr.path().is_ident("doc") {
                match &attr.meta {
                    Meta::NameValue(MetaNameValue {
                        value:
                            Expr::Lit(ExprLit {
                                lit: Lit::Str(token),
                                ..
                            }),
                        ..
                    }) => Some(vec![Ok(quote!(doc = #token))]),
                    _ => Some(vec![Err(ExtractAttrsError::InvalidDocAttrForm)]),
                }
            } else {
                None
            }
        })
        .flatten()
        .map(|attr| attr.map(|attr| quote! { #[#attr] }))
        .collect::<Result<TokenStream, ExtractAttrsError>>()
}

#[derive(Debug, FromVariant)]
struct FragmentVariant {}

#[derive(Debug, FromField)]
#[darling(attributes(fragment), forward_attrs(fragment_attrs, doc))]
struct FragmentField {
    vis: Visibility,
    ident: Option<Ident>,
    ty: Type,
    attrs: Vec<Attribute>,
}

enum Default {
    None,
    FromDefaultTrait,
    Expr(Box<Expr>),
}
impl FromMeta for Default {
    fn from_none() -> Option<Self> {
        Some(Self::None)
    }

    fn from_word() -> darling::Result<Self> {
        Ok(Self::FromDefaultTrait)
    }

    fn from_value(value: &syn::Lit) -> darling::Result<Self> {
        Expr::from_value(value).map(Box::new).map(Self::Expr)
    }
}

pub fn derive(input: DeriveInput) -> TokenStream {
    let FragmentInput {
        ident,
        data,
        attrs,
        mut generics,
        bound,
        path_overrides:
            PathOverrides {
                fragment: fragment_mod,
                result: result_mod,
            },
    } = match FragmentInput::from_derive_input(&input) {
        Ok(input) => input,
        Err(err) => return err.write_errors(),
    };
    let fields = match data {
        Data::Enum(_) => {
            return quote! {
                compile_error!("`#[derive(Fragment)]` does not currently support enums");
            }
        }
        Data::Struct(fields) => fields.fields,
    };

    let fragment_ident = format_ident!("{ident}Fragment");
    let fragment_fields = fields
        .iter()
        .map(
            |FragmentField {
                 vis,
                 ident,
                 ty,
                 attrs,
             }| {
                let attrs = match extract_forwarded_attrs(attrs) {
                    Ok(x) => x,
                    Err(err) => return err.into_compile_error(),
                };
                quote! { #attrs #vis #ident: <#ty as #fragment_mod::FromFragment>::Fragment, }
            },
        )
        .collect::<TokenStream>();

    let from_fragment_fields = fields
        .iter()
        .map(
            |FragmentField {
                 vis: _,
                 ident,
                 ty: _,
                 attrs: _,
             }| {
                let ident_name = ident.as_ref().map(ToString::to_string);
                quote! {
                    #ident: {
                        let validator = validator.field(&#ident_name);
                        #fragment_mod::FromFragment::from_fragment(fragment.#ident, validator)?
                    },
                }
            },
        )
        .collect::<TokenStream>();

    let attrs = match extract_forwarded_attrs(&attrs) {
        Ok(x) => x,
        Err(err) => return err.into_compile_error(),
    };
    if let Some(bound) = bound {
        let where_clause = generics.make_where_clause();
        where_clause.predicates.extend(bound);
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    quote! {
        #attrs
        pub struct #fragment_ident #generics #where_clause {
            #fragment_fields
        }

        impl #impl_generics #fragment_mod::FromFragment for #ident #ty_generics #where_clause {
            type Fragment = #fragment_ident #ty_generics;
            type RequiredFragment = #fragment_ident #ty_generics;

            fn from_fragment(
                fragment: Self::Fragment,
                validator: #fragment_mod::Validator,
            ) -> #result_mod::Result<Self, #fragment_mod::ValidationError> {
                #result_mod::Result::Ok(Self {
                    #from_fragment_fields
                })
            }
        }
    }
}
