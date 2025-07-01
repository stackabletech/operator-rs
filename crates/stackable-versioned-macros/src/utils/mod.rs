use std::ops::Deref;

use convert_case::{Case, Casing};
use darling::util::IdentString;
use k8s_version::Version;
use proc_macro2::Span;
use quote::{ToTokens, format_ident};
use syn::{Ident, Path, spanned::Spanned};

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

pub trait ItemIdentExt: Deref<Target = IdentString> + From<Ident> + Spanned {
    const DEPRECATED_PREFIX: &'static str;

    fn deprecated_prefix(&self) -> &'static str {
        Self::DEPRECATED_PREFIX
    }

    fn starts_with_deprecated_prefix(&self) -> bool {
        self.deref().as_str().starts_with(Self::DEPRECATED_PREFIX)
    }

    /// Removes deprecation prefixed from field or variant idents.
    fn as_cleaned_ident(&self) -> IdentString;
}

pub struct FieldIdent(IdentString);

impl ItemIdentExt for FieldIdent {
    const DEPRECATED_PREFIX: &'static str = "deprecated_";

    fn as_cleaned_ident(&self) -> IdentString {
        self.0
            .clone()
            .map(|i| i.trim_start_matches(Self::DEPRECATED_PREFIX).to_string())
    }
}

impl From<Ident> for FieldIdent {
    fn from(value: Ident) -> Self {
        Self(IdentString::from(value))
    }
}

impl Deref for FieldIdent {
    type Target = IdentString;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ToTokens for FieldIdent {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.to_tokens(tokens);
    }
}

pub struct VariantIdent(IdentString);

impl ItemIdentExt for VariantIdent {
    const DEPRECATED_PREFIX: &'static str = "Deprecated";

    fn as_cleaned_ident(&self) -> IdentString {
        self.0
            .clone()
            .map(|i| i.trim_start_matches(Self::DEPRECATED_PREFIX).to_string())
    }
}

impl From<Ident> for VariantIdent {
    fn from(value: Ident) -> Self {
        Self(IdentString::from(value))
    }
}

impl Deref for VariantIdent {
    type Target = IdentString;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ToTokens for VariantIdent {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.to_tokens(tokens);
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
