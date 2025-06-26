use std::ops::Not;

use darling::{Error, FromAttributes, Result, util::IdentString};
use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Generics, ItemStruct};

use crate::{
    attrs::container::{ContainerAttributes, StructCrdArguments},
    codegen::{
        ItemStatus, VersionDefinition,
        changes::Neighbors,
        container::{
            CommonContainerData, Container, ContainerIdents, ContainerOptions, ContainerTokens,
            Direction, ExtendContainerTokens as _, KubernetesIdents, ModuleGenerationContext,
            VersionContext,
        },
        item::VersionedField,
    },
};

mod conversion;
mod merge;

impl Container {
    pub fn new_struct(item_struct: ItemStruct, versions: &[VersionDefinition]) -> Result<Self> {
        let attributes = ContainerAttributes::from_attributes(&item_struct.attrs)?;

        let mut versioned_fields = Vec::new();
        for field in item_struct.fields {
            let mut versioned_field = VersionedField::new(field, versions)?;
            versioned_field.insert_container_versions(versions);
            versioned_fields.push(versioned_field);
        }

        let idents = ContainerIdents::from(item_struct.ident);

        let kubernetes_data = attributes.crd_arguments.map(|arguments| {
            let idents = KubernetesIdents::from(&idents.original, &arguments);
            KubernetesData {
                kubernetes_arguments: arguments,
                kubernetes_idents: idents,
            }
        });

        // Validate K8s specific requirements
        // Ensure that the struct name includes the 'Spec' suffix.
        if kubernetes_data.is_some() && !idents.original.as_str().ends_with("Spec") {
            return Err(Error::custom(
                "struct name needs to include the `Spec` suffix if Kubernetes features are enabled via `#[versioned(k8s())]`"
            ).with_span(&idents.original.span()));
        }

        let options = ContainerOptions {
            skip_from: attributes.skip.from.is_present(),
            skip_object_from: attributes.skip.object_from.is_present(),
            skip_merged_crd: attributes.skip.merged_crd.is_present(),
            skip_try_convert: attributes.skip.try_convert.is_present(),
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
            kubernetes_data,
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

    pub kubernetes_data: Option<KubernetesData>,

    /// Generic types of the struct
    pub generics: Generics,
}

pub struct KubernetesData {
    pub kubernetes_arguments: StructCrdArguments,
    pub kubernetes_idents: KubernetesIdents,
}

// Common token generation
impl Struct {
    pub fn generate_tokens<'a>(
        &self,
        versions: &'a [VersionDefinition],
        mod_gen_ctx: ModuleGenerationContext<'_>,
    ) -> ContainerTokens<'a> {
        let mut versions_iter = versions.iter().peekable();
        let mut container_tokens = ContainerTokens::default();

        let spec_gen_ctx =
            SpecGenerationContext::new(self.kubernetes_data.as_ref(), versions, mod_gen_ctx);

        while let Some(version) = versions_iter.next() {
            let next_version = versions_iter.peek().copied();
            let ver_ctx = VersionContext::new(version, next_version);

            let struct_definition =
                self.generate_definition(ver_ctx, mod_gen_ctx, spec_gen_ctx.as_ref());

            let upgrade_from = self.generate_from_impl(Direction::Upgrade, ver_ctx, mod_gen_ctx);
            let downgrade_from =
                self.generate_from_impl(Direction::Downgrade, ver_ctx, mod_gen_ctx);

            // Generate code which is only needed for the top-level CRD spec
            if let Some(spec_gen_ctx) = &spec_gen_ctx {
                let upgrade_spec_from = self.generate_object_from_impl(
                    Direction::Upgrade,
                    ver_ctx,
                    mod_gen_ctx,
                    spec_gen_ctx,
                );

                let downgrade_spec_from = self.generate_object_from_impl(
                    Direction::Downgrade,
                    ver_ctx,
                    mod_gen_ctx,
                    spec_gen_ctx,
                );

                container_tokens
                    .extend_between(&version.inner, upgrade_spec_from)
                    .extend_between(&version.inner, downgrade_spec_from);
            }

            container_tokens
                .extend_inner(&version.inner, struct_definition)
                .extend_between(&version.inner, upgrade_from)
                .extend_between(&version.inner, downgrade_from);
        }

        // Generate code which is only needed for the top-level CRD spec
        if let Some(spec_gen_ctx) = spec_gen_ctx {
            let entry_enum = self.generate_entry_enum(mod_gen_ctx, &spec_gen_ctx);
            let entry_enum_impl =
                self.generate_entry_impl_block(versions, mod_gen_ctx, &spec_gen_ctx);
            let version_enum = self.generate_version_enum(mod_gen_ctx, &spec_gen_ctx);
            let status_struct = self.generate_status_struct(mod_gen_ctx, &spec_gen_ctx);

            container_tokens
                .extend_outer(entry_enum)
                .extend_outer(entry_enum_impl)
                .extend_outer(version_enum)
                .extend_outer(status_struct);
        }

        container_tokens
    }

    /// Generates code for the struct definition.
    fn generate_definition(
        &self,
        ver_ctx: VersionContext<'_>,
        mod_gen_ctx: ModuleGenerationContext<'_>,
        spec_gen_ctx: Option<&SpecGenerationContext<'_>>,
    ) -> TokenStream {
        let where_clause = self.generics.where_clause.as_ref();
        let type_generics = &self.generics;

        let original_attributes = &self.common.original_attributes;
        let ident = &self.common.idents.original;
        let version_docs = &ver_ctx.version.docs;

        let fields: TokenStream = self
            .fields
            .iter()
            .filter_map(|field| field.generate_for_container(ver_ctx.version))
            .collect();

        let kube_attribute = spec_gen_ctx.and_then(|spec_gen_ctx| {
            self.generate_kube_attribute(ver_ctx, mod_gen_ctx, spec_gen_ctx)
        });

        quote! {
            #(#[doc = #version_docs])*
            #(#original_attributes)*
            #kube_attribute
            pub struct #ident #type_generics #where_clause {
                #fields
            }
        }
    }

    fn generate_kube_attribute(
        &self,
        ver_ctx: VersionContext<'_>,
        gen_ctx: ModuleGenerationContext<'_>,
        spec_gen_ctx: &SpecGenerationContext<'_>,
    ) -> Option<TokenStream> {
        // Required arguments
        let group = &spec_gen_ctx.kubernetes_arguments.group;
        let version = ver_ctx.version.inner.to_string();
        let kind = spec_gen_ctx
            .kubernetes_arguments
            .kind
            .as_ref()
            .map_or(spec_gen_ctx.kubernetes_idents.kind.to_string(), |kind| {
                kind.clone()
            });

        // Optional arguments
        let singular = spec_gen_ctx
            .kubernetes_arguments
            .singular
            .as_ref()
            .map(|s| quote! { , singular = #s });

        let plural = spec_gen_ctx
            .kubernetes_arguments
            .plural
            .as_ref()
            .map(|p| quote! { , plural = #p });

        let crates = gen_ctx.crates;

        let namespaced = spec_gen_ctx
            .kubernetes_arguments
            .namespaced
            .is_present()
            .then_some(quote! { , namespaced });

        let status = match (
            gen_ctx
                .kubernetes_options
                .experimental_conversion_tracking
                .is_present(),
            &spec_gen_ctx.kubernetes_arguments.status,
        ) {
            (true, _) => {
                let status_ident = &spec_gen_ctx.kubernetes_idents.status;
                Some(quote! { , status = #status_ident })
            }
            (_, Some(status_ident)) => Some(quote! { , status = #status_ident }),
            (_, _) => None,
        };

        let shortnames: TokenStream = spec_gen_ctx
            .kubernetes_arguments
            .shortnames
            .iter()
            .map(|s| quote! { , shortname = #s })
            .collect();

        Some(quote! {
            // The end-developer needs to derive CustomResource and JsonSchema.
            // This is because we don't know if they want to use a re-exported or renamed import.
            #[kube(
                // These must be comma separated (except the last) as they always exist:
                group = #group, version = #version, kind = #kind
                // These fields are optional, and therefore the token stream must prefix each with a comma:
                #singular #plural #namespaced #crates #status #shortnames
            )]
        })
    }

    fn generate_entry_enum(
        &self,
        mod_gen_ctx: ModuleGenerationContext<'_>,
        spec_gen_ctx: &SpecGenerationContext<'_>,
    ) -> TokenStream {
        let enum_ident = &spec_gen_ctx.kubernetes_idents.kind;
        let vis = mod_gen_ctx.vis;

        let variant_idents = &spec_gen_ctx.variant_idents;
        let variant_data = &spec_gen_ctx.variant_data;

        quote! {
            #[derive(Debug)]
            #vis enum #enum_ident {
                #(#variant_idents(#variant_data)),*
            }
        }
    }

    fn generate_entry_impl_block(
        &self,
        versions: &[VersionDefinition],
        mod_gen_ctx: ModuleGenerationContext<'_>,
        spec_gen_ctx: &SpecGenerationContext<'_>,
    ) -> TokenStream {
        let enum_ident = &spec_gen_ctx.kubernetes_idents.kind;

        // Only generate merged_crd associated function if not opted out
        let merged_crd_fn =
            if !mod_gen_ctx.skip.merged_crd.is_present() && !self.common.options.skip_merged_crd {
                Some(self.generate_merged_crd_fn(mod_gen_ctx, spec_gen_ctx))
            } else {
                None
            };

        let try_convert_fn = self.generate_try_convert_fn(versions, mod_gen_ctx, spec_gen_ctx);
        let from_json_value_fn = self.generate_from_json_value_fn(mod_gen_ctx, spec_gen_ctx);
        let into_json_value_fn = self.generate_into_json_value_fn(mod_gen_ctx, spec_gen_ctx);

        quote! {
            impl #enum_ident {
                #merged_crd_fn
                #try_convert_fn
                #from_json_value_fn
                #into_json_value_fn
            }
        }
    }

    fn generate_version_enum(
        &self,
        mod_gen_ctx: ModuleGenerationContext<'_>,
        spec_gen_ctx: &SpecGenerationContext<'_>,
    ) -> Option<TokenStream> {
        if (mod_gen_ctx.skip.merged_crd.is_present() || self.common.options.skip_merged_crd)
            && (mod_gen_ctx.skip.try_convert.is_present() || self.common.options.skip_try_convert)
        {
            return None;
        }

        let enum_ident = &spec_gen_ctx.kubernetes_idents.version;
        let vis = mod_gen_ctx.vis;

        let version_strings = &spec_gen_ctx.version_strings;
        let variant_idents = &spec_gen_ctx.variant_idents;

        Some(quote! {
            #[automatically_derived]
            #vis enum #enum_ident {
                #(#variant_idents),*
            }

            #[automatically_derived]
            impl ::std::fmt::Display for #enum_ident {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::result::Result<(), ::std::fmt::Error> {
                    f.write_str(self.as_str())
                }
            }

            #[automatically_derived]
            impl #enum_ident {
                pub fn as_str(&self) -> &str {
                    match self {
                        #(#enum_ident::#variant_idents => #version_strings),*
                    }
                }
            }
        })
    }

    // Generates the Kubernetes specific From impl for all structs which are part of a spec.
    pub fn generate_from_impl(
        &self,
        direction: Direction,
        ver_ctx: VersionContext<'_>,
        mod_gen_ctx: ModuleGenerationContext<'_>,
    ) -> Option<TokenStream> {
        if mod_gen_ctx.skip.from.is_present() || self.common.options.skip_from {
            return None;
        }

        let next_version = ver_ctx.next_version;
        let version = ver_ctx.version;

        // TODO (@Techassi): The crate overrides need to be applied to the module instead and must
        // be disallowed on individual structs.
        next_version.map(|next_version| {
            // TODO (@Techassi): Support generic types which have been removed in newer versions,
            // but need to exist for older versions How do we represent that? Because the
            // defined struct always represents the latest version. I guess we could generally
            // advise against using generic types, but if you have to, avoid removing it in
            // later versions.
            let (impl_generics, type_generics, where_clause) = self.generics.split_for_impl();
            let from_struct_ident = &self.common.idents.parameter;
            let struct_ident = &self.common.idents.original;

            let version_string = version.inner.to_string();

            let versioned_path = &*mod_gen_ctx.crates.versioned;

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
            let automatically_derived = mod_gen_ctx.add_attributes
                .not()
                .then(|| quote! {#[automatically_derived]});

            let fields = |direction: Direction| -> TokenStream {
                self
                    .fields
                    .iter()
                    .filter_map(|f| {
                        f.generate_for_from_impl(
                            direction,
                            version,
                            next_version,
                            from_struct_ident,
                        )
                    })
                    .collect()
            };

            let inserts: TokenStream = self.fields.iter().filter_map(|f| {
                f.generate_for_status_insertion(direction, next_version, from_struct_ident, mod_gen_ctx)
            }).collect();

            let (fields, for_module_ident, from_module_ident) = match direction {
                Direction::Upgrade => {
                    let from_module_ident = &version.idents.module;
                    let for_module_ident = &next_version.idents.module;

                    (fields(Direction::Upgrade), for_module_ident, from_module_ident)
                }
                Direction::Downgrade => {
                    let from_module_ident = &next_version.idents.module;
                    let for_module_ident = &version.idents.module;

                    (fields(Direction::Downgrade), for_module_ident, from_module_ident)
                }
            };

            // TODO (@Techassi): Re-add support for generics
            // TODO (@Techassi): We know the status, so we can hard-code it, but hard to track across structs

            quote! {
                #automatically_derived
                #allow_attribute
                impl<S> #versioned_path::TrackingFrom<#from_module_ident::#struct_ident, S> for #for_module_ident::#struct_ident
                where
                    S: #versioned_path::TrackingStatus + ::core::default::Default
                {
                    // TODO (@Techassi): Figure out how we can set the correct parent here. Maybe
                    // a map from field name to type and where the type matches _this_ ident.
                    const PARENT: Option<&str> = None;

                    #[allow(unused)]
                    fn tracking_from(#from_struct_ident: #from_module_ident::#struct_ident, status: &S) -> Self {
                        // TODO (@Techassi): Depending on the direction, we need to either insert
                        // changed values into the upgrade or downgrade section. Only then we can
                        // convert the spec.

                        // FIXME (@Techassi): We shouldn't create an entry if we don't need to. This
                        // currently pollutes the status.
                        // TODO (@Techassi): Change the key from a Version to a String to avoid
                        // parsing the version. We know the version is valid, because we previously
                        // parsed it via this macro.
                        let entry = status
                            .changes()
                            .upgrades
                            .entry(#version_string.parse().unwrap())
                            .or_default();

                        #inserts

                        let spec = Self {
                            #fields
                        };

                        // After the spec is converted, depending on the direction, we need to apply
                        // changed values from either the upgrade or downgrade section. Afterwards
                        // we can return the successfully converted spec and the status contains
                        // the tracked changes.

                        spec
                    }
                }
            }
        })
    }

    // Generates the Kubernetes specific From impl for the top-level object.
    pub fn generate_object_from_impl(
        &self,
        direction: Direction,
        ver_ctx: VersionContext<'_>,
        mod_gen_ctx: ModuleGenerationContext<'_>,
        spec_gen_ctx: &SpecGenerationContext<'_>,
    ) -> Option<TokenStream> {
        if mod_gen_ctx.skip.object_from.is_present() || self.common.options.skip_object_from {
            return None;
        }

        let next_version = ver_ctx.next_version;
        let version = ver_ctx.version;

        next_version.map(|next_version| {
            let from_struct_parameter_ident = &spec_gen_ctx.kubernetes_idents.parameter;
            let object_struct_ident = &spec_gen_ctx.kubernetes_idents.kind;
            let spec_struct_ident = &self.common.idents.original;

            let versioned_path = &*mod_gen_ctx.crates.versioned;

            let (for_module_ident, from_module_ident) = match direction {
                Direction::Upgrade => (&next_version.idents.module, &version.idents.module),
                Direction::Downgrade => (&version.idents.module, &next_version.idents.module),
            };

            quote! {
                impl ::std::convert::From<#from_module_ident::#object_struct_ident> for #for_module_ident::#object_struct_ident {
                    fn from(#from_struct_parameter_ident: #from_module_ident::#object_struct_ident) -> Self {
                        // The status is optional. The be able to track changes in nested sub structs it needs
                        // to be initialized with a default value.
                        let mut status = #from_struct_parameter_ident.status.unwrap_or_default();

                        // Convert the spec and track values in the status
                        let spec =
                            <#for_module_ident::#spec_struct_ident as #versioned_path::TrackingFrom<_, _>>::tracking_from(
                                #from_struct_parameter_ident.spec,
                                &status
                            );

                        // Construct the final object by copying over the metadata, setting the status and
                        // using the converted spec.
                        Self {
                            metadata: #from_struct_parameter_ident.metadata,
                            status: Some(status),
                            spec,
                        }
                    }
                }
            }
        })
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

#[derive(Debug)]
pub struct SpecGenerationContext<'a> {
    pub kubernetes_arguments: &'a StructCrdArguments,
    pub kubernetes_idents: &'a KubernetesIdents,

    pub crd_fns: Vec<TokenStream>,
    pub variant_idents: Vec<IdentString>,
    pub variant_data: Vec<TokenStream>,
    pub version_strings: Vec<String>,
}

impl<'a> SpecGenerationContext<'a> {
    pub fn new(
        data: Option<&'a KubernetesData>,
        versions: &[VersionDefinition],
        mod_gen_ctx: ModuleGenerationContext<'_>,
    ) -> Option<Self> {
        match data {
            Some(KubernetesData {
                kubernetes_arguments,
                kubernetes_idents,
            }) => {
                let (crd_fns, variant_idents, variant_data, version_strings) = versions
                    .iter()
                    .map(|version| {
                        Self::generate_version_items(version, mod_gen_ctx, &kubernetes_idents.kind)
                    })
                    .multiunzip::<(Vec<_>, Vec<_>, Vec<_>, Vec<_>)>();

                Some(Self {
                    kubernetes_arguments,
                    kubernetes_idents,
                    crd_fns,
                    variant_idents,
                    variant_data,
                    version_strings,
                })
            }
            None => None,
        }
    }

    fn generate_version_items(
        version: &VersionDefinition,
        mod_gen_ctx: ModuleGenerationContext<'_>,
        struct_ident: &IdentString,
    ) -> (TokenStream, IdentString, TokenStream, String) {
        let module_ident = &version.idents.module;

        let kube_core_path = &*mod_gen_ctx.crates.kube_core;

        let variant_data = quote! { #module_ident::#struct_ident };
        let crd_fn = quote! {
            <#module_ident::#struct_ident as #kube_core_path::CustomResourceExt>::crd()
        };
        let variant_ident = version.idents.variant.clone();
        let version_string = version.inner.to_string();

        (crd_fn, variant_ident, variant_data, version_string)
    }
}
