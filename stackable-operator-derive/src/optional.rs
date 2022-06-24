use darling::{ast::Data, FromDeriveInput, FromField};
use proc_macro2::{Ident, Span};
use quote::{quote, ToTokens};
use syn::{parse_macro_input, Path, Visibility};

#[derive(FromDeriveInput)]
#[darling(attributes(optional), supports(struct_named))]
struct OptionalDeriveInput {
    ident: syn::Ident,
    vis: Visibility,
    data: Data<(), OptionalDeriveField>,
}

#[derive(FromField)]
#[darling(attributes(optional))]
struct OptionalDeriveField {
    ident: Option<syn::Ident>,
    ty: syn::Type,
    vis: Visibility,
    default_value: Option<Path>,
    default_impl: Option<Path>,
}

pub(crate) fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let OptionalDeriveInput { ident, vis, data } =
        match OptionalDeriveInput::from_derive_input(&parse_macro_input!(input)) {
            Ok(input) => input,
            Err(err) => return err.write_errors().into(),
        };

    let fields = match data {
        Data::Struct(fields) => fields.fields,
        _ => {
            return syn::Error::new_spanned(&ident, r#"Enums/Unions can not #[derive(Optional)]"#)
                .to_compile_error()
                .into()
        }
    };

    let original_struct_name = &ident;
    let original_struct_vis = vis;

    let derived_struct_name = Ident::new(
        &format!("Optional{}", original_struct_name),
        Span::call_site(),
    );

    let mut my_fields = quote! {};
    let mut my_impl = quote! {};

    for field in &fields {
        let vis = &field.vis;
        let name = field.ident.as_ref().expect("Unreachable");
        let ty = &field.ty;

        // TODO: This is to identify complex structs (should probably be another attribute - just for testing)
        if &field.default_impl.is_some() {
            my_fields.extend(quote! {
                #vis #name: Complex<#ty>,
            });
        } else {
            my_fields.extend(quote! {
                #vis #name: Option<#ty>,
            });
        }
        let unwrapper = match (&field.default_value, &field.default_impl) {
            (Some(_), Some(_)) =>
            return syn::Error::new_spanned(
                &ident,
                r#"The #[optional(default_value = ...)] and #[optional(default_impl = ...)] attributes are mutually exclusive"#)
            .to_compile_error()
            .into(),
            (Some(value), _) => {
                let value = value.to_token_stream();
                quote! { unwrap_or(#value) }
            }
            (_, Some(method)) => {
                let method = method.to_token_stream();
                quote! { get().unwrap_or_else(|| #method()) }
            }
            (None, None) => quote! { unwrap_or_default() },
        };

        my_impl.extend(quote! {
            #name : c.#name.#unwrapper,
        });
    }

    let struct_optional = quote! {
        // TODO: we should use the derived macros from the original struct and not just hardcode
        #[derive(Clone, Debug, Default, Deserialize, JsonSchema, Merge, PartialEq, Serialize)]
        #[serde(rename_all = "camelCase")]
        #original_struct_vis struct #derived_struct_name {
            #my_fields
        }
    };

    let impl_optional = quote! {
        impl std::convert::From<#derived_struct_name> for #original_struct_name {
             fn from(c: #derived_struct_name) -> Self {
                Self {
                    #my_impl
                }
             }
         }
    };

    let tokens = quote! {
        #struct_optional
        #impl_optional
    };

    eprintln!("Token: {}", tokens);

    tokens.into()
}
