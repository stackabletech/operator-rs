//! GitSync structure for CRDs

use std::{collections::BTreeMap, path::PathBuf};

use schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use stackable_shared::time::Duration;
use url::Url;

use crate::{crd::git_sync::v1alpha2::Credentials, versioned::versioned};

mod v1alpha1_impl;
mod v1alpha2_impl;

#[versioned(version(name = "v1alpha1"), version(name = "v1alpha2"))]
pub mod versioned {
    pub mod v1alpha1 {
        pub use v1alpha1_impl::{Error, GitSyncResources};
    }
    pub mod v1alpha2 {
        pub use v1alpha2_impl::{Error, GitSyncResources};
    }

    #[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct GitSync {
        /// The git repository URL that will be cloned, for example: `https://github.com/stackabletech/airflow-operator` or `ssh://git@github.com:stackable-airflow/dags.git`.
        pub repo: Url,

        /// The branch to clone; defaults to `main`.
        ///
        /// Since git-sync v4.x.x this field is mapped to the flag `--ref`.
        #[serde(default = "v1alpha2::GitSync::default_branch")]
        pub branch: String,

        /// Location in the Git repository containing the resource; defaults to the root folder.
        ///
        /// It can optionally start with `/`, however, no trailing slash is recommended.
        /// An empty string (``) or slash (`/`) corresponds to the root folder in Git.
        #[serde(default = "v1alpha2::GitSync::default_git_folder")]
        pub git_folder: PathBuf,

        /// The depth of syncing, i.e. the number of commits to clone; defaults to 1.
        #[serde(default = "v1alpha2::GitSync::default_depth")]
        pub depth: u32,

        /// The synchronization interval, e.g. `20s` or `5m`; defaults to `20s`.
        ///
        /// Since git-sync v4.x.x this field is mapped to the flag `--period`.
        #[serde(default = "v1alpha2::GitSync::default_wait")]
        pub wait: Duration,

        /// A map of optional configuration settings that are listed in the git-sync [documentation].
        ///
        /// Also read the git-sync [example] in our documentation. These settings are not verified.
        ///
        /// [documentation]: https://github.com/kubernetes/git-sync/tree/v4.2.4?tab=readme-ov-file#manual
        /// [example]: DOCS_BASE_URL_PLACEHOLDER/airflow/usage-guide/mounting-dags#_example
        #[serde(default)]
        pub git_sync_conf: BTreeMap<String, String>,

        /// An optional secret used for git access.
        #[versioned(changed(
            since = "v1alpha2",
            from_name = "credentials_secret",
            from_type = "Option<String>",
            upgrade_with = credentials_secret_to_basic_auth,
            downgrade_with = credentials_to_secret
        ))]
        pub credentials: Option<Credentials>,
    }

    #[derive(strum::Display, Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    #[schemars(rename_all = "camelCase")]
    pub enum Credentials {
        /// The name of the Secret used to access the repository via Basic Authentication if it is not public.
        ///
        /// The referenced Secret must include two fields: `user` and `password`.
        /// The `password` field can either be an actual password (not recommended) or a GitHub token,
        /// as described in the git-sync [documentation].
        ///
        /// [documentation]: https://github.com/kubernetes/git-sync/tree/v4.2.4?tab=readme-ov-file#manual
        BasicAuthSecretName(String),
        /// The name of the Secret used for SSH access to the repository.
        ///
        /// The referenced Secret must include two fields: `key` and `knownHosts`.
        ///
        /// [documentation]: https://github.com/kubernetes/git-sync/tree/v4.2.4?tab=readme-ov-file#manual
        SshPrivateKeySecretName(String),
    }
}

pub fn credentials_to_secret(input: Option<Credentials>) -> Option<String> {
    if let Some(Credentials::BasicAuthSecretName(credentials_secret)) = input {
        Some(credentials_secret)
    } else {
        // We cannot downgrade SshPrivateKeySecretName as it does not map onto
        // anything in v1alpha1
        None
    }
}

pub fn credentials_secret_to_basic_auth(input: Option<String>) -> Option<Credentials> {
    input.map(Credentials::BasicAuthSecretName)
}
