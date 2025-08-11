use darling::util::IdentString;
use k8s_version::Version;

use crate::{
    attrs::module::ModuleAttributes,
    utils::{VersionExt, doc_comments::DocComments},
};

pub mod changes;
pub mod container;
pub mod item;
pub mod module;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionDefinition {
    /// Indicates that the container version is deprecated.
    pub deprecated: Option<String>,

    /// Indicates that the generation of `From<OLD> for NEW` should be skipped.
    pub skip_from: bool,

    /// A validated Kubernetes API version.
    pub inner: Version,

    /// The ident of the container.
    pub idents: VersionIdents,

    /// Store additional doc-comment lines for this version.
    pub docs: Vec<String>,
}

impl PartialOrd for VersionDefinition {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VersionDefinition {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.inner.cmp(&other.inner)
    }
}

impl From<&ModuleAttributes> for Vec<VersionDefinition> {
    fn from(attributes: &ModuleAttributes) -> Self {
        attributes
            .versions
            .iter()
            .map(|v| VersionDefinition {
                skip_from: v.skip.as_ref().is_some_and(|s| s.from.is_present()),
                idents: VersionIdents {
                    module: v.name.as_module_ident(),
                    variant: v.name.as_variant_ident(),
                },
                deprecated: v.deprecated.as_ref().map(|r#override| {
                    r#override
                        .clone()
                        .unwrap_or(format!("Version {version} is deprecated", version = v.name))
                }),
                docs: v.doc.as_deref().into_doc_comments(),
                inner: v.name,
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionIdents {
    pub module: IdentString,
    pub variant: IdentString,
}

#[derive(Clone, Copy, Debug)]
pub struct VersionContext<'a> {
    pub version: &'a VersionDefinition,
    pub next_version: Option<&'a VersionDefinition>,
}

impl<'a> VersionContext<'a> {
    pub fn new(
        version: &'a VersionDefinition,
        next_version: Option<&'a VersionDefinition>,
    ) -> Self {
        Self {
            version,
            next_version,
        }
    }
}

/// Describes the direction of [`From`] implementations.
#[derive(Copy, Clone, Debug)]
pub enum Direction {
    Upgrade,
    Downgrade,
}
