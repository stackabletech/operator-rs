use darling::{
    util::{Flag, SpannedValue},
    Error, FromDeriveInput, FromMeta,
};

#[derive(Debug, FromDeriveInput)]
#[darling(
    attributes(versioned),
    supports(struct_named),
    forward_attrs(allow, doc, cfg, serde),
    and_then = ContainerAttributes::validate
)]
pub(crate) struct ContainerAttributes {
    #[darling(multiple, rename = "version")]
    pub(crate) versions: SpannedValue<Vec<VersionAttributes>>,
}

impl ContainerAttributes {
    fn validate(mut self) -> darling::Result<Self> {
        if self.versions.is_empty() {
            return Err(Error::custom(
                "attribute `#[versioned()]` must contain at least one `version`",
            )
            .with_span(&self.versions.span()));
        }

        for version in &mut *self.versions {
            if version.name.is_empty() {
                return Err(Error::custom("field `name` of `version` must not be empty")
                    .with_span(&version.name.span()));
            }

            if !version
                .name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
            {
                return Err(Error::custom(
                    "field `name` of `version` must only contain alphanumeric ASCII characters (a-z, A-Z, 0-9, '.', '-')",
                )
                .with_span(&version.name.span()));
            }
        }

        Ok(self)
    }
}

#[derive(Debug, FromMeta)]
pub struct VersionAttributes {
    pub(crate) name: SpannedValue<String>,

    // TODO (@Techassi): Remove the rename when the field uses the correct name
    #[darling(rename = "deprecated")]
    pub(crate) _deprecated: Flag,
}
