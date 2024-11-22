use darling::{util::Flag, FromMeta};

use crate::attrs::common::CommonRootArguments;

#[derive(Debug, FromMeta)]
pub(crate) struct ModuleAttributes {
    #[darling(flatten)]
    pub(crate) common_root_arguments: CommonRootArguments,
    pub(crate) preserve_module: Flag,
}
