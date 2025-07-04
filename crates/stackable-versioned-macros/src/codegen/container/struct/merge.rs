use proc_macro2::TokenStream;
use quote::quote;

use crate::codegen::container::{
    ModuleGenerationContext,
    r#struct::{SpecGenerationContext, Struct},
};

impl Struct {
    pub(super) fn generate_merged_crd_fn(
        &self,
        mod_gen_ctx: ModuleGenerationContext<'_>,
        spec_gen_ctx: &SpecGenerationContext<'_>,
    ) -> Option<TokenStream> {
        if mod_gen_ctx.skip.merged_crd.is_present() || self.common.options.skip_merged_crd {
            return None;
        }

        // Get various idents needed for code generation
        let version_enum_ident = &spec_gen_ctx.kubernetes_idents.version;

        // Get the crate paths
        let k8s_openapi_path = &*mod_gen_ctx.crates.k8s_openapi;
        let kube_core_path = &*mod_gen_ctx.crates.kube_core;

        let crd_fns = &spec_gen_ctx.crd_fns;

        Some(quote! {
            /// Generates a merged CRD containing all versions and marking `stored_apiversion` as stored.
            pub fn merged_crd(
                stored_apiversion: #version_enum_ident
            ) -> ::std::result::Result<
                #k8s_openapi_path::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
                #kube_core_path::crd::MergeError>
            {
                #kube_core_path::crd::merge_crds(vec![#(#crd_fns),*], stored_apiversion.as_version_str())
            }
        })
    }
}
