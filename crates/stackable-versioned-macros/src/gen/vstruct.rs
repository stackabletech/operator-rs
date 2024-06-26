use darling::FromField;
use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Attribute, DataStruct, Error, Ident, Result};

use crate::{
    attrs::{container::ContainerAttributes, field::FieldAttributes},
    gen::{field::VersionedField, version::ContainerVersion},
};

/// Stores individual versions of a single struct. Each version tracks field
/// actions, which describe if the field was added, renamed or deprecated in
/// that version. Fields which are not versioned, are included in every
/// version of the struct.
#[derive(Debug)]
pub(crate) struct VersionedStruct {
    /// The ident, or name, of the versioned struct.
    pub(crate) ident: Ident,

    /// The name of the struct used in `From` implementations.
    pub(crate) from_ident: Ident,

    /// List of declared versions for this struct. Each version, except the
    /// latest, generates a definition with appropriate fields.
    pub(crate) versions: Vec<ContainerVersion>,

    /// List of fields defined in the base struct. How, and if, a field should
    /// generate code, is decided by the currently generated version.
    pub(crate) fields: Vec<VersionedField>,

    pub(crate) skip_from: bool,

    /// The original attributes that were added to the struct.
    pub(crate) original_attrs: Vec<Attribute>,
}

impl VersionedStruct {
    pub(crate) fn new(
        ident: Ident,
        data: DataStruct,
        attributes: ContainerAttributes,
        original_attrs: Vec<Attribute>,
    ) -> Result<Self> {
        // Convert the raw version attributes into a container version.
        let versions = attributes
            .versions
            .iter()
            .map(|v| ContainerVersion {
                skip_from: v.skip.as_ref().map_or(false, |s| s.from.is_present()),
                ident: format_ident!("{version}", version = v.name.to_string()),
                deprecated: v.deprecated.is_present(),
                doc: v.doc.clone(),
                inner: v.name,
            })
            .collect();

        // Extract the field attributes for every field from the raw token
        // stream and also validate that each field action version uses a
        // version declared by the container attribute.
        let mut fields = Vec::new();

        for field in data.fields {
            let attrs = FieldAttributes::from_field(&field)?;
            attrs.validate_versions(&attributes, &field)?;

            let mut versioned_field = VersionedField::new(field, attrs)?;
            versioned_field.insert_container_versions(&versions);
            fields.push(versioned_field);
        }

        // Check for field ident collisions
        for version in &versions {
            // Collect the idents of all fields for a single version and then
            // ensure that all idents are unique. If they are not, return an
            // error.
            let mut idents = Vec::new();

            // TODO (@Techassi): Report which field(s) use a duplicate ident and
            // also hint what can be done to fix it based on the field action /
            // status.

            for field in &fields {
                idents.push(field.get_ident(version))
            }

            if !idents.iter().all_unique() {
                return Err(Error::new(
                    ident.span(),
                    format!("struct contains renamed fields which collide with other fields in version {version}", version = version.inner),
                ));
            }
        }

        let from_ident = format_ident!("__sv_{ident}", ident = ident.to_string().to_lowercase());

        Ok(Self {
            skip_from: attributes
                .options
                .skip
                .map_or(false, |s| s.from.is_present()),
            from_ident,
            versions,
            fields,
            ident,
            original_attrs,
        })
    }

    /// This generates the complete code for a single versioned struct.
    ///
    /// Internally, it will create a module for each declared version which
    /// contains the struct with the appropriate fields. Additionally, it
    /// generated `From` implementations, which enable conversion from an older
    /// to a newer version.
    pub(crate) fn generate_tokens(&self) -> TokenStream {
        let mut token_stream = TokenStream::new();
        let mut versions = self.versions.iter().peekable();

        while let Some(version) = versions.next() {
            token_stream.extend(self.generate_version(version, versions.peek().copied()));
        }

        token_stream
    }

    fn generate_version(
        &self,
        version: &ContainerVersion,
        next_version: Option<&ContainerVersion>,
    ) -> TokenStream {
        let mut token_stream = TokenStream::new();
        let struct_name = &self.ident;

        // Generate fields of the struct for `version`.
        let fields = self.generate_struct_fields(version);

        // TODO (@Techassi): Make the generation of the module optional to
        // enable the attribute macro to be applied to a module which
        // generates versioned versions of all contained containers.

        let deprecated_attr = version.deprecated.then_some(quote! {#[deprecated]});
        let module_name = &version.ident;
        let attrs = &self.original_attrs;
        let doc = if let Some(doc) = &version.doc {
            let doc = format!("Docs for `{module_name}`: {doc}");
            Some(quote! {
                #[doc = ""]
                #[doc = #doc]
            })
        } else {
            None
        };

        // Generate tokens for the module and the contained struct
        token_stream.extend(quote! {
            #[automatically_derived]
            #deprecated_attr
            pub mod #module_name {
                #(#attrs)*
                #doc
                pub struct #struct_name {
                    #fields
                }
            }
        });

        // Generate the From impl between this `version` and the next one.
        if !self.skip_from && !version.skip_from {
            token_stream.extend(self.generate_from_impl(version, next_version));
        }

        token_stream
    }

    fn generate_struct_fields(&self, version: &ContainerVersion) -> TokenStream {
        let mut token_stream = TokenStream::new();

        for field in &self.fields {
            token_stream.extend(field.generate_for_struct(version));
        }

        token_stream
    }

    fn generate_from_impl(
        &self,
        version: &ContainerVersion,
        next_version: Option<&ContainerVersion>,
    ) -> TokenStream {
        if let Some(next_version) = next_version {
            let next_module_name = &next_version.ident;
            let from_ident = &self.from_ident;
            let module_name = &version.ident;
            let struct_name = &self.ident;

            let fields = self.generate_from_fields(version, next_version, from_ident);

            // TODO (@Techassi): Be a little bit more clever about when to include
            // the #[allow(deprecated)] attribute.
            return quote! {
                #[automatically_derived]
                #[allow(deprecated)]
                impl From<#module_name::#struct_name> for #next_module_name::#struct_name {
                    fn from(#from_ident: #module_name::#struct_name) -> Self {
                        Self {
                            #fields
                        }
                    }
                }
            };
        }

        quote! {}
    }

    fn generate_from_fields(
        &self,
        version: &ContainerVersion,
        next_version: &ContainerVersion,
        from_ident: &Ident,
    ) -> TokenStream {
        let mut token_stream = TokenStream::new();

        for field in &self.fields {
            token_stream.extend(field.generate_for_from_impl(version, next_version, from_ident))
        }

        token_stream
    }
}
