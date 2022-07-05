use darling::{ast::Data, FromDeriveInput, FromField, FromMeta, FromVariant};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{DeriveInput, Expr, GenericArgument, Type};

#[derive(FromDeriveInput)]
#[darling(attributes(fragment))]
pub struct FragmentInput {
    ident: Ident,
    data: Data<FragmentVariant, FragmentField>,
}

#[derive(FromVariant)]
struct FragmentVariant {}

#[derive(FromField)]
#[darling(attributes(fragment))]
struct FragmentField {
    ident: Option<Ident>,
    ty: Type,
    default: Default,
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
    let FragmentInput { ident, data } = match FragmentInput::from_derive_input(&input) {
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
             }| {
                let ty = extract_inner_option_type(ty).unwrap_or(ty);
                quote! { #ident: Option<<#ty as FromFragment>::Fragment>, }
            },
        )
        .collect::<TokenStream>();

    let from_fragment_fields = fields
        .iter()
        .map(|FragmentField { ident, ty, default }| {
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
                Default::FromDefaultTrait => quote! { Some(Default::default()) },
                Default::None => quote! { None },
            };
            let mut fragment_value = quote! { fragment.#ident.or_else(|| #default_fragment_value) };
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
                    let value = FromFragment::from_fragment(value, validator)?;
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
        })
        .collect::<TokenStream>();

    quote! {
        #[derive(Default)]
        struct #fragment_ident {
            #fragment_fields
        }

        impl FromFragment for #ident {
            type Fragment = #fragment_ident;

            fn from_fragment(fragment: Self::Fragment, validator: Validator) -> Result<Self, ValidationError> {
                Ok(Self {
                    #from_fragment_fields
                })
            }

            fn default_fragment() -> Option<Self::Fragment> {
                Some(#fragment_ident::default())
            }
        }
    }
}
