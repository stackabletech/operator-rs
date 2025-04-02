use darling::{FromMeta, util::Flag};

use crate::attrs::common::{CommonOptions, CommonRootArguments, SkipArguments};

#[derive(Debug, FromMeta)]
pub(crate) struct ModuleAttributes {
    #[darling(flatten)]
    pub(crate) common: CommonRootArguments<ModuleOptions>,
}

#[derive(Debug, FromMeta, Default)]
pub(crate) struct ModuleOptions {
    pub(crate) allow_unsorted: Flag,
    pub(crate) skip: Option<SkipArguments>,
    pub(crate) preserve_module: Flag,
}

impl CommonOptions for ModuleOptions {
    fn allow_unsorted(&self) -> Flag {
        self.allow_unsorted
    }
}
