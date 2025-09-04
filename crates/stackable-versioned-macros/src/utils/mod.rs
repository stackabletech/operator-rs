use convert_case::{Case, Casing};
use darling::util::IdentString;
use k8s_version::Version;
use proc_macro2::Span;
use quote::format_ident;
use syn::{Ident, Path};

pub mod doc_comments;

pub trait VersionExt {
    fn as_variant_ident(&self) -> IdentString;
    fn as_module_ident(&self) -> IdentString;
}

impl VersionExt for Version {
    fn as_variant_ident(&self) -> IdentString {
        IdentString::new(Ident::new(
            &self.to_string().to_case(Case::Pascal),
            Span::call_site(),
        ))
    }

    fn as_module_ident(&self) -> IdentString {
        IdentString::new(Ident::new(&self.to_string(), Span::call_site()))
    }
}

/// Provides extra functionality on top of [`IdentString`]s used to name containers.
pub trait ContainerIdentExt {
    /// Removes the 'Spec' suffix from the [`IdentString`].
    fn as_cleaned_kubernetes_ident(&self) -> IdentString;

    /// Transforms the [`IdentString`] into one usable in the [`From`] impl.
    fn as_parameter_ident(&self) -> IdentString;
}

impl ContainerIdentExt for Ident {
    fn as_cleaned_kubernetes_ident(&self) -> IdentString {
        let ident = format_ident!("{}", self.to_string().trim_end_matches("Spec"));
        IdentString::new(ident)
    }

    fn as_parameter_ident(&self) -> IdentString {
        let ident = format_ident!("__sv_{}", self.to_string().to_lowercase());
        IdentString::new(ident)
    }
}

impl ContainerIdentExt for IdentString {
    fn as_cleaned_kubernetes_ident(&self) -> IdentString {
        self.as_ident().as_cleaned_kubernetes_ident()
    }

    fn as_parameter_ident(&self) -> IdentString {
        self.as_ident().as_parameter_ident()
    }
}

pub trait ItemIdents {
    const DEPRECATION_PREFIX: &str;

    fn deprecation_prefix(&self) -> &str {
        Self::DEPRECATION_PREFIX
    }

    fn starts_with_deprecation_prefix(&self) -> bool {
        self.original()
            .as_str()
            .starts_with(Self::DEPRECATION_PREFIX)
    }

    fn cleaned(&self) -> &IdentString;
    fn original(&self) -> &IdentString;
}

pub trait ItemIdentExt {
    fn json_path_ident(&self) -> IdentString;
}

impl ItemIdentExt for IdentString {
    fn json_path_ident(&self) -> IdentString {
        format_ident!("__sv_{}_path", self.as_str().to_lowercase()).into()
    }
}

pub fn path_to_string(path: &Path) -> String {
    let pretty_path = path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<String>>()
        .join("::");

    match path.leading_colon {
        Some(_) => format!("::{}", pretty_path),
        None => pretty_path,
    }
}
