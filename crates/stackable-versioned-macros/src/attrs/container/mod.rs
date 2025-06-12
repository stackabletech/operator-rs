use darling::{Error, FromAttributes, FromMeta, Result, util::Flag};

use crate::attrs::{
    common::{CommonOptions, CommonRootArguments, SkipArguments},
    container::k8s::KubernetesArguments,
};

pub mod k8s;

#[derive(Debug, FromMeta)]
#[darling(and_then = StandaloneContainerAttributes::validate)]
pub struct StandaloneContainerAttributes {
    #[darling(rename = "k8s")]
    pub kubernetes_arguments: Option<KubernetesArguments>,

    #[darling(flatten)]
    pub common: CommonRootArguments<StandaloneContainerOptions>,
}

impl StandaloneContainerAttributes {
    fn validate(self) -> Result<Self> {
        if self.kubernetes_arguments.is_some() && cfg!(not(feature = "k8s")) {
            return Err(Error::custom(
                "the `#[versioned(k8s())]` attribute can only be used when the `k8s` feature is enabled",
            ));
        }

        Ok(self)
    }
}

#[derive(Debug, FromMeta, Default)]
pub struct StandaloneContainerOptions {
    pub allow_unsorted: Flag,
    pub skip: Option<SkipArguments>,
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
pub struct NestedContainerAttributes {
    #[darling(rename = "k8s")]
    pub kubernetes_arguments: Option<KubernetesArguments>,

    #[darling(default)]
    pub options: NestedContainerOptionArguments,
}

impl NestedContainerAttributes {
    fn validate(self) -> Result<Self> {
        if self.kubernetes_arguments.is_some() && cfg!(not(feature = "k8s")) {
            return Err(Error::custom(
                "the `#[versioned(k8s())]` attribute can only be used when the `k8s` feature is enabled",
            ));
        }

        Ok(self)
    }
}

#[derive(Debug, Default, FromMeta)]
pub struct NestedContainerOptionArguments {
    pub skip: Option<SkipArguments>,
}
