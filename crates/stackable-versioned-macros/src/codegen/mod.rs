use darling::util::IdentString;
use k8s_version::Version;
use proc_macro2::TokenStream;
use syn::{Path, Type};

use crate::{
    attrs::{container::StandaloneContainerAttributes, module::ModuleAttributes},
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

// NOTE (@Techassi): Can we maybe unify these two impls?
impl From<&StandaloneContainerAttributes> for Vec<VersionDefinition> {
    fn from(attributes: &StandaloneContainerAttributes) -> Self {
        attributes
            .common
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

impl From<&ModuleAttributes> for Vec<VersionDefinition> {
    fn from(attributes: &ModuleAttributes) -> Self {
        attributes
            .common
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

#[derive(Debug, PartialEq)]
pub enum ItemStatus {
    Addition {
        ident: IdentString,
        default_fn: Path,
        // NOTE (@Techassi): We need to carry idents and type information in
        // nearly every status. Ideally, we would store this in separate maps.
        ty: Type,
    },
    Change {
        downgrade_with: Option<Path>,
        upgrade_with: Option<Path>,
        from_ident: IdentString,
        to_ident: IdentString,
        from_type: Type,
        to_type: Type,
    },
    Deprecation {
        previous_ident: IdentString,
        note: Option<String>,
        ident: IdentString,
    },
    NoChange {
        previously_deprecated: bool,
        ident: IdentString,
        ty: Type,
    },
    NotPresent,
}

impl ItemStatus {
    pub fn get_ident(&self) -> &IdentString {
        match &self {
            ItemStatus::Addition { ident, .. } => ident,
            ItemStatus::Change { to_ident, .. } => to_ident,
            ItemStatus::Deprecation { ident, .. } => ident,
            ItemStatus::NoChange { ident, .. } => ident,
            ItemStatus::NotPresent => unreachable!("ItemStatus::NotPresent does not have an ident"),
        }
    }
}

// This contains all generated Kubernetes tokens for a particular version.
// This struct can then be used to fully generate the combined final Kubernetes code.
#[derive(Debug, Default)]
pub struct KubernetesTokens {
    variant_idents: Vec<IdentString>,
    variant_data: Vec<TokenStream>,
    variant_strings: Vec<String>,
    crd_fns: Vec<TokenStream>,
}

impl KubernetesTokens {
    pub fn push(&mut self, items: (TokenStream, IdentString, TokenStream, String)) {
        self.crd_fns.push(items.0);
        self.variant_idents.push(items.1);
        self.variant_data.push(items.2);
        self.variant_strings.push(items.3);
    }
}
