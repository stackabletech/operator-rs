use darling::{ast::Data, FromDeriveInput, FromField};
use proc_macro2::{Ident, Span};
use quote::{quote, ToTokens};
use syn::{parse_macro_input, Path, Visibility};

#[derive(FromDeriveInput)]
#[darling(attributes(config), supports(struct_named))]
struct ConfigReceiver {
    ident: syn::Ident,
    vis: Visibility,
    data: Data<(), ConfigFieldReceiver>,
}

#[derive(FromField)]
#[darling(attributes(config))]
struct ConfigFieldReceiver {
    ident: Option<syn::Ident>,
    ty: syn::Type,
    vis: Visibility,
    default_value: Option<String>,
    default_impl: Option<Path>,
}

pub(crate) fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ConfigReceiver { ident, vis, data } =
        match ConfigReceiver::from_derive_input(&parse_macro_input!(input)) {
            Ok(input) => input,
            Err(err) => return err.write_errors().into(),
        };

    let fields = match data {
        Data::Struct(fields) => fields.fields,
        _ => {
            return syn::Error::new_spanned(&ident, r#"Enums/Unions can not #[derive(Config)]"#)
                .to_compile_error()
                .into()
        }
    };

    let original_struct_name = &ident;
    let original_struct_vis = vis;

    let mergable_name = Ident::new(
        &format!("Mergable{}", original_struct_name.to_string()),
        Span::call_site(),
    );

    let mut my_fields = quote! {};
    let mut my_impl = quote! {};

    for field in &fields {
        let vis = &field.vis;
        let name = field.ident.as_ref().expect("Unreachable");
        let ty = &field.ty;

        my_fields.extend(quote! {
            #vis #name: Option<#ty>,
        });

        let unwrapper = match (&field.default_value, &field.default_impl) {
            (Some(_), Some(_)) =>
            return syn::Error::new_spanned(
                &ident,
                r#"The #[config(default_value = ...)] and #[config(default_impl = ...)] attributes are mutually exclusive"#)
            .to_compile_error()
            .into(),
            (Some(val), _) => {
                let value = Ident::new(val, Span::call_site());
                quote! { unwrap_or(#value) }
            }
            (_, Some(path)) => {
                let method = path.to_token_stream();
                quote! { unwrap_or_else(|| #method()) }
            }
            (None, None) => quote! { unwrap_or_default() },
        };

        my_impl.extend(quote! {
            #name : c.#name.#unwrapper,
        });
    }

    // Concat output
    let struct_mergable = quote! {
        #[derive(Merge)]
        #original_struct_vis struct #mergable_name {
            #my_fields
        }
    };

    let impl_mergable = quote! {
        impl std::convert::From<#mergable_name> for #original_struct_name {
             fn from(c: #mergable_name) -> Self {
                Self {
                    #my_impl
                }
             }
         }
    };

    let tokens = quote! {
        #struct_mergable
        #impl_mergable
    };

    eprintln!("TOKENS: {}", tokens);

    tokens.into()
}
