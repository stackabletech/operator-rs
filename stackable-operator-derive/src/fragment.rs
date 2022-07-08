use darling::{ast::Data, FromDeriveInput, FromField, FromMeta, FromVariant};
use proc_macro2::{Ident, TokenStream, TokenTree};
use quote::{format_ident, quote};
use syn::{parse_quote, Attribute, DeriveInput, Expr, GenericArgument, Generics, Path, Type};

#[derive(FromMeta)]
struct PathOverrides {
    #[darling(default = "PathOverrides::default_fragment")]
    fragment: Path,
    #[darling(default = "PathOverrides::default_default")]
    default: Path,
    #[darling(default = "PathOverrides::default_result")]
    result: Path,
}
impl std::default::Default for PathOverrides {
    fn default() -> Self {
        Self {
            fragment: Self::default_fragment(),
            default: Self::default_default(),
            result: Self::default_result(),
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

fn extract_inner_option_type(ty: &Type) -> Option<&Type> {
    let path = if let Type::Path(path) = ty {
        path
    } else {
        return None;
    };
    let seg = only(&path.path.segments)?;
    if seg.ident != "Option" {
        return None;
    }
    let args = if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
        args
    } else {
        return None;
    };
    let arg = only(&args.args)?;
    if let GenericArgument::Type(arg_ty) = arg {
        Some(arg_ty)
    } else {
        None
    }
}

pub fn derive(input: DeriveInput) -> TokenStream {
    let FragmentInput {
        ident,
        data,
        attrs,
        generics,
        path_overrides:
            PathOverrides {
                fragment: fragment_mod,
                default: default_mod,
                result: result_mod,
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
                let ty = extract_inner_option_type(ty).unwrap_or(ty);
                let attrs = extract_forwarded_attrs(attrs);
                quote! { #attrs #ident: Option<<#ty as #fragment_mod::FromFragment>::Fragment>, }
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
                let inner_option_ty = extract_inner_option_type(ty);
                let is_option_wrapped = inner_option_ty.is_some();
                let ty = inner_option_ty.unwrap_or(ty);
                let wrapped_value = if is_option_wrapped {
                    quote! { Some(value) }
                } else {
                    quote! { value }
                };
                let default_fragment_value = match default {
                    Default::Expr(default) => quote! { Some(#default) },
                    Default::FromDefaultTrait => quote! { Some(#default_mod::Default::default()) },
                    Default::None => quote! { None },
                };
                let mut fragment_value =
                    quote! { fragment.#ident.or_else(|| #default_fragment_value) };
                if !is_option_wrapped {
                    fragment_value = quote! { #fragment_value.or_else(#ty::default_fragment) };
                }
                let default_value = if is_option_wrapped {
                    quote! { None }
                } else {
                    quote! { return Err(validator.error_required()) }
                };
                let value = quote! {
                    if let Some(value) = #fragment_value {
                        let value = #fragment_mod::FromFragment::from_fragment(value, validator)?;
                        #wrapped_value
                    } else {
                        #default_value
                    }
                };
                quote! {
                    #ident: {
                        let validator = validator.field(#ident_name);
                        #value
                    },
                }
            },
        )
        .collect::<TokenStream>();

    let fragment_field_defaults = fields
        .iter()
        .map(|FragmentField { ident, .. }| quote! { #ident: None, })
        .collect::<TokenStream>();

    let attrs = extract_forwarded_attrs(&attrs);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    quote! {
        #attrs
        struct #fragment_ident #impl_generics #where_clause {
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

            fn from_fragment(
                fragment: <Self as #fragment_mod::FromFragment>::Fragment,
                validator: #fragment_mod::Validator,
            ) -> #result_mod::Result<Self, #fragment_mod::ValidationError> {
                #result_mod::Result::Ok(Self {
                    #from_fragment_fields
                })
            }

            fn default_fragment() -> Option<<Self as #fragment_mod::FromFragment>::Fragment> {
                Some(<Self as #fragment_mod::FromFragment>::Fragment::default())
            }
        }
    }
}
