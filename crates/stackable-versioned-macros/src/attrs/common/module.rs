use darling::{
    util::{Flag, SpannedValue},
    FromMeta, Result,
};

use crate::attrs::common::{SkipArguments, VersionArguments};

#[derive(Debug, FromMeta)]
#[darling(and_then = ModuleAttributes::validate)]
pub(crate) struct ModuleAttributes {
    #[darling(multiple, rename = "version")]
    pub(crate) versions: SpannedValue<Vec<VersionArguments>>,

    #[darling(default, rename = "options")]
    pub(crate) common_option_args: ModuleOptionArguments,
}

impl ModuleAttributes {
    fn validate(self) -> Result<Self> {
        // TODO (@Techassi): Make this actually validate
        Ok(self)
    }
}

#[derive(Debug, Default, FromMeta)]
pub(crate) struct ModuleOptionArguments {
    pub(crate) skip: Option<SkipArguments>,
    pub(crate) preserve_module: Flag,
    pub(crate) allow_unsorted: Flag,
}
