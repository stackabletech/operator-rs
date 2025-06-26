use darling::{Error, FromAttributes, FromMeta, Result, util::Flag};
use syn::Path;

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
        if self.crd_arguments.is_some()
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
///    name (without the 'Spec' suffix).
/// - `singular`: Set the singular name of the CR object.
/// - `plural`: Set the plural name of the CR object.
/// - `namespaced`: Indicate that this is a namespaced scoped resource rather than a
///    cluster scoped resource.
/// - `crates`: Override specific crates.
/// - `status`: Set the specified struct as the status subresource.
/// - `shortname`: Set a shortname for the CR object. This can be specified multiple
///   times.
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
    // scale
    // printcolumn
    #[darling(multiple, rename = "shortname")]
    pub shortnames: Vec<String>,
    // category
    // selectable
    // doc
    // annotation
    // label
}
