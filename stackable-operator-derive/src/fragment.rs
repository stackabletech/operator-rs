use darling::{ast::Data, FromDeriveInput, FromField, FromMeta, FromVariant};
use proc_macro2::{Ident, TokenStream, TokenTree};
use quote::{format_ident, quote};
use syn::{parse_quote, Attribute, DeriveInput, Expr, Generics, Path, Type, WherePredicate};

#[derive(FromMeta)]
struct PathOverrides {
    #[darling(default = "PathOverrides::default_fragment")]
    fragment: Path,
    #[darling(default = "PathOverrides::default_default")]
    default: Path,
    #[darling(default = "PathOverrides::default_result")]
    result: Path,
    #[darling(default = "PathOverrides::default_option")]
    option: Path,
}
impl std::default::Default for PathOverrides {
    fn default() -> Self {
        Self {
            fragment: Self::default_fragment(),
            default: Self::default_default(),
            result: Self::default_result(),
            option: Self::default_option(),
        }
    }
}
impl PathOverrides {
    fn default_fragment() -> Path {
        parse_quote!(::stackable_operator::config::fragment)
    }

    fn default_default() -> Path {
        parse_quote!(::core::default)
    }

    fn default_result() -> Path {
        parse_quote!(::core::result)
    }

    fn default_option() -> Path {
        parse_quote!(::core::option)
    }
}

#[derive(FromDeriveInput)]
#[darling(attributes(fragment), forward_attrs(fragment_attrs))]
pub struct FragmentInput {
    ident: Ident,
    generics: Generics,
    data: Data<FragmentVariant, FragmentField>,
    attrs: Vec<Attribute>,
    #[darling(default)]
    path_overrides: PathOverrides,
    #[darling(default)]
    bounds: Option<Vec<WherePredicate>>,
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

fn extract_forwarded_attrs(attrs: &[Attribute]) -> TokenStream {
    attrs
        .iter()
        .filter(|attr| attr.path.is_ident("fragment_attrs"))
        .flat_map(|Attribute { tokens, .. }| match only(tokens.clone()) {
            Some(TokenTree::Group(group)) => split_by_comma(group.stream()),
            _ => todo!(),
        })
        .flat_map(|attr| {
            quote! { #[#attr] }
        })
        .collect()
}

#[derive(FromVariant)]
struct FragmentVariant {}

#[derive(FromField)]
#[darling(attributes(fragment), forward_attrs(fragment_attrs))]
struct FragmentField {
    ident: Option<Ident>,
    ty: Type,
    default: Default,
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

fn only<I: IntoIterator>(iter: I) -> Option<I::Item> {
    let mut iter = iter.into_iter();
    let item = iter.next()?;
    if iter.next().is_some() {
        None
    } else {
        Some(item)
    }
}

pub fn derive(input: DeriveInput) -> TokenStream {
    let FragmentInput {
        ident,
        data,
        attrs,
        mut generics,
        bounds,
        path_overrides:
            PathOverrides {
                fragment: fragment_mod,
                default: default_mod,
                result: result_mod,
                option: option_mod,
            },
    } = match FragmentInput::from_derive_input(&input) {
        Ok(input) => input,
        Err(err) => return err.write_errors(),
    };
    let fields = match data {
        Data::Enum(_) => todo!(),
        Data::Struct(fields) => fields.fields,
    };

    let fragment_ident = format_ident!("{ident}Fragment");
    let fragment_fields = fields
        .iter()
        .map(
            |FragmentField {
                 ident,
                 ty,
                 default: _,
                 attrs,
             }| {
                let attrs = extract_forwarded_attrs(attrs);
                quote! { #attrs #ident: <#ty as #fragment_mod::FromFragment>::OptionalFragment, }
            },
        )
        .collect::<TokenStream>();

    let from_fragment_fields = fields
        .iter()
        .map(
            |FragmentField {
                 ident,
                 ty,
                 default,
                 attrs: _,
             }| {
                let ident_name = ident.as_ref().map(ToString::to_string);
                let default_fragment_value = match default {
                    Default::Expr(default) => quote! { Some(#default) },
                    Default::FromDefaultTrait => quote! { Some(#default_mod::Default::default()) },
                    Default::None => quote! { None },
                };
                quote! {
                    #ident: {
                        let validator = validator.field(#ident_name);
                        let fragment_value = <#ty>::or_default_fragment(
                            #fragment_mod::Optional::or_else(
                                fragment.#ident,
                                || #default_fragment_value,
                            )
                        );
                        if let #option_mod::Option::Some(value) = fragment_value {
                            #fragment_mod::FromFragment::from_fragment(value, validator)?
                        } else {
                            return Err(validator.error_required())
                        }
                    },
                }
            },
        )
        .collect::<TokenStream>();

    let fragment_field_defaults = fields
        .iter()
        .map(|FragmentField { ident, .. }| quote! { #ident: #fragment_mod::Optional::none(), })
        .collect::<TokenStream>();

    let attrs = extract_forwarded_attrs(&attrs);
    if let Some(bounds) = bounds {
        let where_clause = generics.make_where_clause();
        where_clause.predicates.extend(bounds);
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    quote! {
        #attrs
        pub struct #fragment_ident #impl_generics #where_clause {
            #fragment_fields
        }

        impl #impl_generics #default_mod::Default for #fragment_ident #ty_generics #where_clause {
            fn default() -> Self {
                Self {
                    #fragment_field_defaults
                }
            }
        }

        impl #impl_generics #fragment_mod::FromFragment for #ident #ty_generics #where_clause {
            type Fragment = #fragment_ident #ty_generics;
            type OptionalFragment = Option<#fragment_ident #ty_generics>;

            fn from_fragment(
                fragment: Self::Fragment,
                validator: #fragment_mod::Validator,
            ) -> #result_mod::Result<Self, #fragment_mod::ValidationError> {
                #result_mod::Result::Ok(Self {
                    #from_fragment_fields
                })
            }

            fn or_default_fragment(opt: Self::OptionalFragment) -> Option<Self::Fragment> {
                Some(opt.unwrap_or_else(|| Self::Fragment::default()))
            }
        }
    }
}
