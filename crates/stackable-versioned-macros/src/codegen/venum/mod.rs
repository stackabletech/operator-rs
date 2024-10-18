use std::ops::Deref;

use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{punctuated::Punctuated, token::Comma, Error, Variant};

use crate::{
    attrs::common::StandaloneContainerAttributes,
    codegen::{
        chain::Neighbors,
        common::{
            generate_module, Container, ContainerInput, Item, ItemStatus, VersionDefinition,
            VersionedContainer,
        },
        venum::variant::VersionedVariant,
    },
};

pub(crate) mod variant;

pub(crate) struct GenerateVersionTokens {
    from_impl: Option<TokenStream>,
    enum_definition: TokenStream,
}

/// Stores individual versions of a single enum. Each version tracks variant
/// actions, which describe if the variant was added, renamed or deprecated in
/// that version. Variants which are not versioned, are included in every
/// version of the enum.
#[derive(Debug)]
pub(crate) struct VersionedEnum(VersionedContainer<VersionedVariant>);

impl Deref for VersionedEnum {
    type Target = VersionedContainer<VersionedVariant>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Container<Punctuated<Variant, Comma>, VersionedVariant> for VersionedEnum {
    fn new(
        input: ContainerInput,
        variants: Punctuated<Variant, Comma>,
        attributes: StandaloneContainerAttributes,
    ) -> syn::Result<Self> {
        let ident = &input.ident;

        // Convert the raw version attributes into a container version.
        let versions: Vec<_> = (&attributes).into();

        // Extract the attributes for every variant from the raw token
        // stream and also validate that each variant action version uses a
        // version declared by the container attribute.
        let mut items = Vec::new();

        for variant in variants {
            let mut versioned_field = VersionedVariant::new(variant, &attributes)?;
            versioned_field.insert_container_versions(&versions);
            items.push(versioned_field);
        }

        // Check for field ident collisions
        for version in &versions {
            // Collect the idents of all variants for a single version and then
            // ensure that all idents are unique. If they are not, return an
            // error.

            // TODO (@Techassi): Report which variant(s) use a duplicate ident and
            // also hint what can be done to fix it based on the variant action /
            // status.

            if !items.iter().map(|f| f.get_ident(version)).all_unique() {
                return Err(Error::new(
                    ident.span(),
                    format!("Enum contains renamed variants which collide with other variants in version {version}", version = version.inner),
                ));
            }
        }

        Ok(Self(VersionedContainer::new(
            input, attributes, versions, items,
        )))
    }

    fn generate_standalone_tokens(&self) -> TokenStream {
        let mut tokens = TokenStream::new();
        let mut versions = self.versions.iter().peekable();

        while let Some(version) = versions.next() {
            let GenerateVersionTokens {
                enum_definition,
                from_impl,
            } = self.generate_version(version, versions.peek().copied());

            let module_definition = generate_module(version, &self.visibility, enum_definition);

            tokens.extend(module_definition);
            tokens.extend(from_impl);
        }

        tokens
    }

    fn generate_nested_tokens(&self) -> TokenStream {
        quote! {}
    }
}

impl VersionedEnum {
    fn generate_version(
        &self,
        version: &VersionDefinition,
        next_version: Option<&VersionDefinition>,
    ) -> GenerateVersionTokens {
        let mut enum_definition = TokenStream::new();

        let original_attributes = &self.original_attributes;
        let enum_name = &self.idents.original;

        // Generate variants of the enum for `version`.
        let variants = self.generate_enum_variants(version);

        // Generate doc comments for the container (enum)
        let version_specific_docs = self.generate_enum_docs(version);

        // Generate enum definition tokens
        enum_definition.extend(quote! {
            #version_specific_docs
            #(#original_attributes)*
            pub enum #enum_name {
                #variants
            }
        });

        let from_impl = if !self.options.skip_from && !version.skip_from {
            self.generate_from_impl(version, next_version)
        } else {
            None
        };

        GenerateVersionTokens {
            enum_definition,
            from_impl,
        }
    }

    /// Generates version specific doc comments for the enum.
    fn generate_enum_docs(&self, version: &VersionDefinition) -> TokenStream {
        let mut tokens = TokenStream::new();

        for (i, doc) in version.version_specific_docs.iter().enumerate() {
            if i == 0 {
                // Prepend an empty line to clearly separate the version
                // specific docs.
                tokens.extend(quote! {
                    #[doc = ""]
                })
            }
            tokens.extend(quote! {
                #[doc = #doc]
            })
        }

        tokens
    }

    fn generate_enum_variants(&self, version: &VersionDefinition) -> TokenStream {
        let mut token_stream = TokenStream::new();

        for variant in &self.items {
            token_stream.extend(variant.generate_for_container(version));
        }

        token_stream
    }

    fn generate_from_impl(
        &self,
        version: &VersionDefinition,
        next_version: Option<&VersionDefinition>,
    ) -> Option<TokenStream> {
        if let Some(next_version) = next_version {
            let next_module_name = &next_version.ident;
            let module_name = &version.ident;

            let enum_ident = &self.idents.original;
            let from_ident = &self.idents.from;

            let mut variants = TokenStream::new();

            for item in &self.items {
                variants.extend(item.generate_for_from_impl(
                    module_name,
                    next_module_name,
                    version,
                    next_version,
                    enum_ident,
                ))
            }

            // Include allow(deprecated) only when this or the next version is
            // deprecated. Also include it, when a variant in this or the next
            // version is deprecated.
            let allow_attribute = (version.deprecated
                || next_version.deprecated
                || self.is_any_variant_deprecated(version)
                || self.is_any_variant_deprecated(next_version))
            .then_some(quote! { #[allow(deprecated)] });

            return Some(quote! {
                #[automatically_derived]
                #allow_attribute
                impl ::std::convert::From<#module_name::#enum_ident> for #next_module_name::#enum_ident {
                    fn from(#from_ident: #module_name::#enum_ident) -> Self {
                        match #from_ident {
                            #variants
                        }
                    }
                }
            });
        }

        None
    }

    /// Returns whether any field is deprecated in the provided
    /// [`ContainerVersion`].
    fn is_any_variant_deprecated(&self, version: &VersionDefinition) -> bool {
        // First, iterate over all fields. Any will return true if any of the
        // function invocations return true. If a field doesn't have a chain,
        // we can safely default to false (unversioned fields cannot be
        // deprecated). Then we retrieve the status of the field and ensure it
        // is deprecated.
        self.items.iter().any(|f| {
            f.chain.as_ref().map_or(false, |c| {
                c.value_is(&version.inner, |a| {
                    matches!(
                        a,
                        ItemStatus::Deprecation { .. }
                            | ItemStatus::NoChange {
                                previously_deprecated: true,
                                ..
                            }
                    )
                })
            })
        })
    }
}
