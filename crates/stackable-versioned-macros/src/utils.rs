use std::ops::Deref;

use convert_case::{Case, Casing};
use darling::util::IdentString;
use k8s_version::Version;
use quote::{format_ident, ToTokens};
use syn::{spanned::Spanned, Ident};

pub(crate) trait VersionExt {
    fn as_variant_ident(&self) -> IdentString;
}

impl VersionExt for Version {
    fn as_variant_ident(&self) -> IdentString {
        format_ident!("{ident}", ident = self.to_string().to_case(Case::Pascal)).into()
    }
}

/// Provides extra functionality on top of [`IdentString`]s used to name containers.
pub(crate) trait ContainerIdentExt {
    /// Removes the 'Spec' suffix from the [`IdentString`].
    fn as_cleaned_kubernetes_ident(&self) -> IdentString;

    /// Transforms the [`IdentString`] into one usable in the [`From`] impl.
    fn as_from_impl_ident(&self) -> IdentString;
}

impl ContainerIdentExt for Ident {
    fn as_cleaned_kubernetes_ident(&self) -> IdentString {
        let ident = format_ident!("{}", self.to_string().trim_end_matches("Spec"));
        IdentString::new(ident)
    }

    fn as_from_impl_ident(&self) -> IdentString {
        let ident = format_ident!("__sv_{}", self.to_string().to_lowercase());
        IdentString::new(ident)
    }
}

pub(crate) trait ItemIdentExt: Deref<Target = IdentString> + From<Ident> + Spanned {
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

pub(crate) struct FieldIdent(IdentString);

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

pub(crate) struct VariantIdent(IdentString);

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
