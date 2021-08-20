extern crate proc_macro;
extern crate syn;
#[macro_use]
extern crate quote;

use proc_macro::TokenStream;
use syn::parse2;

#[proc_macro_derive(Restart)]
pub fn restart(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast = syn::parse(input).unwrap();

    // Build the trait implementation
    impl_restart(&ast)
}

#[proc_macro_attribute]
pub fn command(args: TokenStream, input: TokenStream) -> TokenStream {
    let x = format!(
        r#"
        fn dummy() {{
            println!("entering");
            println!("args tokens: {{}}", {args});
            println!("input tokens: {{}}", {input});
            println!("exiting");
        }}
    "#,
        args = args.into_iter().count(),
        input = input.into_iter().count(),
    );

    x.parse().expect("Generated invalid tokens")
}

#[proc_macro_derive(Rolling)]
pub fn rolling(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast = syn::parse(input).unwrap();

    // Build the trait implementation
    impl_rolling(&ast)
}

fn impl_restart(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let gen = quote! {
        impl Command for Stop {
            fn get_owner_name(&self) -> String {
                self.spec.name.clone()
            }

            fn start(&mut self) {
                self.spec.started_at = Some(Utc::now().to_rfc3339());
            }

            fn done(&mut self) {
                self.spec.finished_at = Some(Utc::now().to_rfc3339());
            }

            fn start_time(&self) -> Option<DateTime<FixedOffset>> {
                self.spec
                    .started_at
                    .as_ref()
                    .map(|time_string| DateTime::<FixedOffset>::parse_from_rfc3339(time_string).unwrap())
            }

            fn get_start_patch(&self) -> Value {
                json!({
                    "spec": {
                        "startedAt": &self.spec.started_at
                    }
                })
            }
        }
    };
    gen.into()
}

fn impl_rolling(ast: &syn::DeriveInput) -> TokenStream {
    let gen = quote! {
        impl CanBeRolling for Stop {
            fn is_rolling(&self) -> bool {
                self.spec.rolling
            }
        }
    };
    gen.into()
}
