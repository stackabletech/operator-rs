use darling::{Error, FromAttributes, FromMeta, Result, util::Flag};
use syn::{Meta, Path};

#[derive(Debug, FromAttributes)]
#[darling(attributes(versioned), and_then = ContainerAttributes::validate)]
pub struct ContainerAttributes {
    #[darling(rename = "crd")]
    pub crd_arguments: Option<StructCrdArguments>,

    #[darling(default)]
    pub skip: ContainerSkipArguments,
}

impl ContainerAttributes {
    fn validate(self) -> Result<Self> {
        if self.crd_arguments.is_none()
            && (self.skip.object_from.is_present()
                || self.skip.merged_crd.is_present()
                || self.skip.try_convert.is_present())
        {
            return Err(Error::custom("spec sub structs can only use skip(from)"));
        }

        Ok(self)
    }
}

#[derive(Debug, Default, FromMeta)]
pub struct ContainerSkipArguments {
    pub from: Flag,
    pub object_from: Flag,
    pub merged_crd: Flag,
    pub try_convert: Flag,
}

/// This struct contains supported CRD arguments.
///
/// The arguments are passed through to the `#[kube]` attribute. More details can be found in the
/// official docs: <https://docs.rs/kube/latest/kube/derive.CustomResource.html>.
///
/// Supported arguments are:
///
/// - `group`: Set the group of the CR object, usually the domain of the company.
///   This argument is Required.
/// - `kind`: Override the kind field of the CR object. This defaults to the struct
///   name (without the 'Spec' suffix).
/// - `singular`: Set the singular name of the CR object.
/// - `plural`: Set the plural name of the CR object.
/// - `namespaced`: Indicate that this is a namespaced scoped resource rather than a
///   cluster scoped resource.
/// - `crates`: Override specific crates.
/// - `status`: Set the specified struct as the status subresource.
/// - `scale`: Configure the scale subresource for horizontal pod autoscaling integration.
/// - `printcolumn`: Add an additional printer column, which is displayed by `kubectl get` and
///   also nicely shows up in tools like k9s. This can be specified multiple times. The argument is
///   passed through as-is, so it supports the same syntax as `#[kube(printcolumn(...))]`.
/// - `shortname`: Set a shortname for the CR object. This can be specified multiple
///   times.
/// - `selectable`: Set the JSON path of a field which can be used as a field selector.
///   This can be specified multiple times.
/// - `doc`: Set the root description of the generated CRD. This argument is required, so
///   that every CRD has a meaningful description instead of the generic one that kube
///   generates by default.
/// - `skip`: Controls skipping parts of the generation.
#[derive(Clone, Debug, FromMeta)]
pub struct StructCrdArguments {
    pub group: String,
    pub kind: Option<String>,
    pub singular: Option<String>,
    pub plural: Option<String>,
    pub namespaced: Flag,
    // root
    pub status: Option<Path>,
    // derive
    // schema
    pub scale: Option<Scale>,
    // Kept as raw meta and passed through verbatim, so that kube-derive does the parsing and
    // validation and we don't need to keep a copy of its printcolumn struct in sync.
    #[darling(multiple, rename = "printcolumn")]
    pub printcolumns: Vec<Meta>,
    #[darling(multiple, rename = "shortname")]
    pub shortnames: Vec<String>,
    // category
    #[darling(multiple, rename = "selectable")]
    pub selectable_fields: Vec<String>,
    pub doc: String,
    // annotation
    // label
}

/// Scale subresource configuration for a CRD.
///
/// Mirrors the fields of [`k8s_openapi::CustomResourceSubresourceScale`][1] and what is present in
/// `kube_derive`.
///
/// [1]: k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceSubresourceScale
//
// TODO (@Techassi): This should eventually get replaced by directly using what `kube_derive` offers,
// but that requires an upstream restructure I'm planning to do soon(ish).
#[expect(clippy::struct_field_names)]
#[derive(Clone, Debug, FromMeta)]
pub struct Scale {
    pub spec_replicas_path: String,
    pub status_replicas_path: String,

    #[darling(default)]
    pub label_selector_path: Option<String>,
}
