use std::ops::Not;

use darling::{Error, FromAttributes, Result};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Generics, ItemStruct};

use crate::{
    attrs::container::NestedContainerAttributes,
    codegen::{
        ItemStatus, StandaloneContainerAttributes, VersionDefinition,
        changes::Neighbors,
        container::{CommonContainerData, Container, ContainerIdents, ContainerOptions},
        item::VersionedField,
    },
};

mod k8s;

impl Container {
    pub fn new_standalone_struct(
        item_struct: ItemStruct,
        attributes: StandaloneContainerAttributes,
        versions: &[VersionDefinition],
    ) -> Result<Self> {
        // NOTE (@Techassi): Should we check if the fields are named here?
        let mut versioned_fields = Vec::new();

        for field in item_struct.fields {
            let mut versioned_field = VersionedField::new(field, versions)?;
            versioned_field.insert_container_versions(versions);
            versioned_fields.push(versioned_field);
        }

        let kubernetes_arguments = attributes.kubernetes_arguments;
        let idents = ContainerIdents::from(item_struct.ident, kubernetes_arguments.as_ref());

        // Validate K8s specific requirements
        // Ensure that the struct name includes the 'Spec' suffix.
        if kubernetes_arguments.is_some() && !idents.original.as_str().ends_with("Spec") {
            return Err(Error::custom(
                "struct name needs to include the `Spec` suffix if Kubernetes features are enabled via `#[versioned(k8s())]`"
            ).with_span(&idents.original.span()));
        }

        let options = ContainerOptions {
            skip_from: attributes
                .common
                .options
                .skip
                .is_some_and(|s| s.from.is_present()),
            kubernetes_arguments,
        };

        let common = CommonContainerData {
            original_attributes: item_struct.attrs,
            options,
            idents,
        };

        Ok(Self::Struct(Struct {
            generics: item_struct.generics,
            fields: versioned_fields,
            common,
        }))
    }

    // TODO (@Techassi): See what can be unified into a single 'new' function
    pub fn new_struct_nested(
        item_struct: ItemStruct,
        versions: &[VersionDefinition],
    ) -> Result<Self> {
        let attributes = NestedContainerAttributes::from_attributes(&item_struct.attrs)?;

        let mut versioned_fields = Vec::new();
        for field in item_struct.fields {
            let mut versioned_field = VersionedField::new(field, versions)?;
            versioned_field.insert_container_versions(versions);
            versioned_fields.push(versioned_field);
        }

        let kubernetes_arguments = attributes.kubernetes_arguments;
        let idents = ContainerIdents::from(item_struct.ident, kubernetes_arguments.as_ref());

        // Validate K8s specific requirements
        // Ensure that the struct name includes the 'Spec' suffix.
        if kubernetes_arguments.is_some() && !idents.original.as_str().ends_with("Spec") {
            return Err(Error::custom(
                "struct name needs to include the `Spec` suffix if Kubernetes features are enabled via `#[versioned(k8s())]`"
            ).with_span(&idents.original.span()));
        }

        let options = ContainerOptions {
            skip_from: attributes.options.skip.is_some_and(|s| s.from.is_present()),
            kubernetes_arguments,
        };

        // Nested structs
        // We need to filter out the `versioned` attribute, because these are not directly processed
        // by darling, but instead by us (using darling). For this reason, darling won't remove the
        // attribute from the input and as such, we need to filter it out ourself.
        let original_attributes = item_struct
            .attrs
            .into_iter()
            .filter(|attr| !attr.meta.path().is_ident("versioned"))
            .collect();

        let common = CommonContainerData {
            original_attributes,
            options,
            idents,
        };

        Ok(Self::Struct(Struct {
            generics: item_struct.generics,
            fields: versioned_fields,
            common,
        }))
    }
}

/// A versioned struct.
pub struct Struct {
    /// List of fields defined in the original struct. How, and if, an item
    /// should generate code, is decided by the currently generated version.
    pub fields: Vec<VersionedField>,

    /// Common container data which is shared between structs and enums.
    pub common: CommonContainerData,

    /// Generic types of the struct
    pub generics: Generics,
}

// Common token generation
impl Struct {
    /// Generates code for the struct definition.
    pub fn generate_definition(&self, version: &VersionDefinition) -> TokenStream {
        let where_clause = self.generics.where_clause.as_ref();
        let type_generics = &self.generics;

        let original_attributes = &self.common.original_attributes;
        let ident = &self.common.idents.original;
        let version_docs = &version.docs;

        let mut fields = TokenStream::new();
        for field in &self.fields {
            fields.extend(field.generate_for_container(version));
        }

        // This only returns Some, if K8s features are enabled
        let kube_attribute = self.generate_kube_attribute(version);

        quote! {
            #(#[doc = #version_docs])*
            #(#original_attributes)*
            #kube_attribute
            pub struct #ident #type_generics #where_clause {
                #fields
            }
        }
    }

    // TODO (@Techassi): It looks like some of the stuff from the upgrade and downgrade functions
    // can be combined into a single piece of code. Figure out a nice way to do that.
    /// Generates code for the `From<Version> for NextVersion` implementation.
    ///
    /// The `add_attributes` parameter declares if attributes (macros) should be added to the
    /// generated `From` impl block.
    pub fn generate_upgrade_from_impl(
        &self,
        version: &VersionDefinition,
        next_version: Option<&VersionDefinition>,
        add_attributes: bool,
    ) -> Option<TokenStream> {
        if version.skip_from || self.common.options.skip_from {
            return None;
        }

        match next_version {
            Some(next_version) => {
                // TODO (@Techassi): Support generic types which have been removed in newer versions,
                // but need to exist for older versions How do we represent that? Because the
                // defined struct always represents the latest version. I guess we could generally
                // advise against using generic types, but if you have to, avoid removing it in
                // later versions.
                let (impl_generics, type_generics, where_clause) = self.generics.split_for_impl();
                let from_struct_ident = &self.common.idents.parameter;
                let struct_ident = &self.common.idents.original;

                let for_module_ident = &next_version.idents.module;
                let from_module_ident = &version.idents.module;

                let fields: TokenStream = self
                    .fields
                    .iter()
                    .map(|f| {
                        f.generate_for_upgrade_from_impl(version, next_version, from_struct_ident)
                    })
                    .collect();

                // Include allow(deprecated) only when this or the next version is
                // deprecated. Also include it, when a field in this or the next
                // version is deprecated.
                let allow_attribute = (version.deprecated.is_some()
                    || next_version.deprecated.is_some()
                    || self.is_any_field_deprecated(version)
                    || self.is_any_field_deprecated(next_version))
                .then(|| quote! { #[allow(deprecated)] });

                // Only add the #[automatically_derived] attribute only if this impl is used
                // outside of a module (in standalone mode).
                let automatically_derived = add_attributes
                    .not()
                    .then(|| quote! {#[automatically_derived]});

                Some(quote! {
                    #automatically_derived
                    #allow_attribute
                    impl #impl_generics ::std::convert::From<#from_module_ident::#struct_ident #type_generics> for #for_module_ident::#struct_ident #type_generics
                        #where_clause
                    {
                        fn from(#from_struct_ident: #from_module_ident::#struct_ident #type_generics) -> Self {
                            Self {
                                #fields
                            }
                        }
                    }
                })
            }
            None => None,
        }
    }

    pub fn generate_downgrade_from_impl(
        &self,
        version: &VersionDefinition,
        next_version: Option<&VersionDefinition>,
        add_attributes: bool,
    ) -> Option<TokenStream> {
        if version.skip_from || self.common.options.skip_from {
            return None;
        }

        match next_version {
            Some(next_version) => {
                let (impl_generics, type_generics, where_clause) = self.generics.split_for_impl();
                let from_struct_ident = &self.common.idents.parameter;
                let struct_ident = &self.common.idents.original;

                let from_module_ident = &next_version.idents.module;
                let for_module_ident = &version.idents.module;

                let fields: TokenStream = self
                    .fields
                    .iter()
                    .map(|f| {
                        f.generate_for_downgrade_from_impl(version, next_version, from_struct_ident)
                    })
                    .collect();

                // Include allow(deprecated) only when this or the next version is
                // deprecated. Also include it, when a field in this or the next
                // version is deprecated.
                let allow_attribute = (version.deprecated.is_some()
                    || next_version.deprecated.is_some()
                    || self.is_any_field_deprecated(version)
                    || self.is_any_field_deprecated(next_version))
                .then(|| quote! { #[allow(deprecated)] });

                // Only add the #[automatically_derived] attribute only if this impl is used
                // outside of a module (in standalone mode).
                let automatically_derived = add_attributes
                    .not()
                    .then(|| quote! {#[automatically_derived]});

                Some(quote! {
                    #automatically_derived
                    #allow_attribute
                    impl #impl_generics ::std::convert::From<#from_module_ident::#struct_ident #type_generics> for #for_module_ident::#struct_ident #type_generics
                        #where_clause
                    {
                        fn from(#from_struct_ident: #from_module_ident::#struct_ident #type_generics) -> Self {
                            Self {
                                #fields
                            }
                        }
                    }
                })
            }
            None => None,
        }
    }

    /// Returns whether any field is deprecated in the provided `version`.
    fn is_any_field_deprecated(&self, version: &VersionDefinition) -> bool {
        // First, iterate over all fields. The `any` function will return true
        // if any of the function invocations return true. If a field doesn't
        // have a chain, we can safely default to false (unversioned fields
        // cannot be deprecated). Then we retrieve the status of the field and
        // ensure it is deprecated.
        self.fields.iter().any(|f| {
            f.changes.as_ref().is_some_and(|c| {
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
