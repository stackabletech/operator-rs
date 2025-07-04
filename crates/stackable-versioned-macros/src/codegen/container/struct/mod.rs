use darling::{Error, FromAttributes, Result, util::IdentString};
use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Generics, ItemStruct};

use crate::{
    attrs::container::{ContainerAttributes, StructCrdArguments},
    codegen::{
        Direction, VersionContext, VersionDefinition,
        changes::Neighbors,
        container::{
            CommonContainerData, Container, ContainerIdents, ContainerOptions, ContainerTokens,
            ExtendContainerTokens as _, KubernetesIdents, ModuleGenerationContext,
        },
        item::{ItemStatus, VersionedField},
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
                "struct name needs to include the `Spec` suffix if CRD features are enabled via `#[versioned(crd())]`"
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
        mod_gen_ctx: ModuleGenerationContext<'_>,
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

        let crates = mod_gen_ctx.crates;

        let namespaced = spec_gen_ctx
            .kubernetes_arguments
            .namespaced
            .is_present()
            .then_some(quote! { , namespaced });

        // NOTE (@Techassi): What an abomination
        let status = match (
            mod_gen_ctx
                .kubernetes_options
                .experimental_conversion_tracking
                .is_present(),
            &spec_gen_ctx.kubernetes_arguments.status,
        ) {
            (true, status_path) => {
                if (mod_gen_ctx.skip.merged_crd.is_present() || self.common.options.skip_merged_crd)
                    && (mod_gen_ctx.skip.try_convert.is_present()
                        || self.common.options.skip_try_convert)
                {
                    status_path
                        .as_ref()
                        .map(|status_path| quote! { , status = #status_path })
                } else {
                    let status_ident = &spec_gen_ctx.kubernetes_idents.status;
                    Some(quote! { , status = #status_ident })
                }
            }
            (false, Some(status_path)) => Some(quote! { , status = #status_path }),
            _ => None,
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

        let automatically_derived = mod_gen_ctx.automatically_derived_attr();

        quote! {
            #automatically_derived
            #[derive(::core::fmt::Debug)]
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

        let merged_crd_fn = self.generate_merged_crd_fn(mod_gen_ctx, spec_gen_ctx);
        let try_convert_fn = self.generate_try_convert_fn(versions, mod_gen_ctx, spec_gen_ctx);
        let from_json_value_fn = self.generate_from_json_value_fn(mod_gen_ctx, spec_gen_ctx);
        let into_json_value_fn = self.generate_into_json_value_fn(mod_gen_ctx, spec_gen_ctx);

        let automatically_derived = mod_gen_ctx.automatically_derived_attr();

        quote! {
            #automatically_derived
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

        let versioned_path = &*mod_gen_ctx.crates.versioned;
        let unknown_desired_api_version_error =
            quote! { #versioned_path::UnknownDesiredApiVersionError };

        let version_strings = &spec_gen_ctx.version_strings;
        let variant_idents = &spec_gen_ctx.variant_idents;

        let automatically_derived = mod_gen_ctx.automatically_derived_attr();

        // TODO (@Techassi): Generate this once
        let api_versions = version_strings
            .iter()
            .map(|version| {
                format!(
                    "{group}/{version}",
                    group = &spec_gen_ctx.kubernetes_arguments.group
                )
            })
            .collect::<Vec<_>>();

        Some(quote! {
            #automatically_derived
            #[derive(
                ::core::marker::Copy,
                ::core::clone::Clone,
                ::core::fmt::Debug
            )]
            #vis enum #enum_ident {
                #(#variant_idents),*
            }

            #automatically_derived
            impl ::core::fmt::Display for #enum_ident {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::std::result::Result<(), ::std::fmt::Error> {
                    // The version (without the Kubernetes group) is probably more human-readable
                    f.write_str(self.as_version_str())
                }
            }

            #automatically_derived
            impl #enum_ident {
                pub fn as_version_str(&self) -> &str {
                    match self {
                        #(#enum_ident::#variant_idents => #version_strings),*
                    }
                }

                pub fn as_api_version_str(&self) -> &str {
                    match self {
                        #(#enum_ident::#variant_idents => #api_versions),*
                    }
                }

                pub fn from_api_version(api_version: &str) -> Result<Self, #unknown_desired_api_version_error> {
                    match api_version {
                        #(#api_versions => Ok(#enum_ident::#variant_idents)),*,
                        _ => Err(#unknown_desired_api_version_error {
                            api_version: api_version.to_owned(),
                        }),
                    }
                }
            }
        })
    }

    /// Generates the Kubernetes specific From impl for all structs which are part of a spec.
    fn generate_from_impl(
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

        next_version.map(|next_version| {
            if mod_gen_ctx
                .kubernetes_options
                .experimental_conversion_tracking
                .is_present()
            {
                self.generate_tracking_from_impl(direction, version, next_version, mod_gen_ctx)
            } else {
                self.generate_plain_from_impl(direction, version, next_version, mod_gen_ctx)
            }
        })
    }

    fn generate_plain_from_impl(
        &self,
        direction: Direction,
        version: &VersionDefinition,
        next_version: &VersionDefinition,
        mod_gen_ctx: ModuleGenerationContext<'_>,
    ) -> TokenStream {
        // TODO (@Techassi): A bunch this stuff is duplicated in self.generate_tracking_from_impl.
        // Ideally we remove that duplication.
        let from_struct_ident = &self.common.idents.parameter;
        let struct_ident = &self.common.idents.original;

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
        let automatically_derived = mod_gen_ctx.automatically_derived_attr();

        let fields = |direction: Direction| -> TokenStream {
            self.fields
                .iter()
                .filter_map(|f| {
                    f.generate_for_from_impl(direction, version, next_version, from_struct_ident)
                })
                .collect()
        };

        let (fields, for_module_ident, from_module_ident) = match direction {
            direction @ Direction::Upgrade => {
                let from_module_ident = &version.idents.module;
                let for_module_ident = &next_version.idents.module;

                (fields(direction), for_module_ident, from_module_ident)
            }
            direction @ Direction::Downgrade => {
                let from_module_ident = &next_version.idents.module;
                let for_module_ident = &version.idents.module;

                (fields(direction), for_module_ident, from_module_ident)
            }
        };

        quote! {
            #automatically_derived
            #allow_attribute
            impl ::core::convert::From<#from_module_ident::#struct_ident> for #for_module_ident::#struct_ident {
                fn from(#from_struct_ident: #from_module_ident::#struct_ident) -> Self {
                    Self {
                        #fields
                    }
                }
            }
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
                c.value_is(&version.inner, |s| {
                    matches!(
                        s,
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
