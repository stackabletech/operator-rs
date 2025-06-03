use darling::{FromMeta, util::Flag};

use crate::attrs::common::{CommonOptions, CommonRootArguments, SkipArguments};

#[derive(Debug, FromMeta)]
pub struct ModuleAttributes {
    #[darling(flatten)]
    pub common: CommonRootArguments<ModuleOptions>,
}

#[derive(Debug, FromMeta, Default)]
pub struct ModuleOptions {
    pub allow_unsorted: Flag,
    pub skip: Option<SkipArguments>,
    pub preserve_module: Flag,
}

impl CommonOptions for ModuleOptions {
    fn allow_unsorted(&self) -> Flag {
        self.allow_unsorted
    }
}
