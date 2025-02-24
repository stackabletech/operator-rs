use darling::{util::Flag, Error, FromAttributes, FromMeta, Result};

use crate::attrs::{
    common::{CommonOptions, CommonRootArguments, SkipArguments},
    k8s::KubernetesArguments,
};

#[derive(Debug, FromMeta)]
#[darling(and_then = StandaloneContainerAttributes::validate)]
pub(crate) struct StandaloneContainerAttributes {
    #[darling(rename = "k8s")]
    pub(crate) kubernetes_arguments: Option<KubernetesArguments>,

    #[darling(flatten)]
    pub(crate) common: CommonRootArguments<StandaloneContainerOptions>,
}

impl StandaloneContainerAttributes {
    fn validate(self) -> Result<Self> {
        if self.kubernetes_arguments.is_some() && cfg!(not(feature = "k8s")) {
            return Err(Error::custom("the `#[versioned(k8s())]` attribute can only be used when the `k8s` feature is enabled"));
        }

        Ok(self)
    }
}

#[derive(Debug, FromMeta, Default)]
pub(crate) struct StandaloneContainerOptions {
    pub(crate) allow_unsorted: Flag,
    pub(crate) skip: Option<SkipArguments>,
}

impl CommonOptions for StandaloneContainerOptions {
    fn allow_unsorted(&self) -> Flag {
        self.allow_unsorted
    }
}

#[derive(Debug, FromAttributes)]
#[darling(
    attributes(versioned),
    and_then = NestedContainerAttributes::validate
)]
pub(crate) struct NestedContainerAttributes {
    #[darling(rename = "k8s")]
    pub(crate) kubernetes_arguments: Option<KubernetesArguments>,

    #[darling(default)]
    pub(crate) options: NestedContainerOptionArguments,
}

impl NestedContainerAttributes {
    fn validate(self) -> Result<Self> {
        if self.kubernetes_arguments.is_some() && cfg!(not(feature = "k8s")) {
            return Err(Error::custom("the `#[versioned(k8s())]` attribute can only be used when the `k8s` feature is enabled"));
        }

        Ok(self)
    }
}

#[derive(Debug, Default, FromMeta)]
pub(crate) struct NestedContainerOptionArguments {
    pub(crate) skip: Option<SkipArguments>,
}
