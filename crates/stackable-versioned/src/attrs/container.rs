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
    #[darling(multiple)]
    pub(crate) version: SpannedValue<Vec<VersionAttributes>>,
}

impl ContainerAttributes {
    fn validate(mut self) -> darling::Result<Self> {
        if self.version.is_empty() {
            return Err(Error::custom(
                "attribute `#[versioned()]` must contain at least one `version`",
            )
            .with_span(&self.version.span()));
        }

        for version in &mut *self.version {
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

            // TODO (@Techassi): Use Diagnostics API when stablizized to throw
            // a warning when the input mismatches the generated module name.
            // See https://github.com/rust-lang/rust/issues/54140
            let module_name = version.name.to_lowercase();
            if module_name != *version.name {
                println!("the generated module name differs from the provided version name which might cause confusion around what the code seems to suggest")
            }
            version.module_name = module_name
        }

        Ok(self)
    }
}

#[derive(Debug, FromMeta)]
pub struct VersionAttributes {
    pub(crate) name: SpannedValue<String>,

    pub(crate) deprecated: Flag,

    #[darling(skip)]
    pub(crate) module_name: String,
    // #[darling(default = default_visibility)]
    // pub(crate) visibility: Visibility,
}
