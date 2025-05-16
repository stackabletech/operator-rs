//! GitSync structure for CRDs

use std::{collections::BTreeMap, path::PathBuf};

use schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{time::Duration, versioned::versioned};

mod v1alpha1_impl;

#[versioned(version(name = "v1alpha1"))]
pub mod versioned {
    pub mod v1alpha1 {
        pub use v1alpha1_impl::{Error, GitSyncResources};
    }

    #[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct GitSync {
        /// The git repository URL that will be cloned, for example: `https://github.com/stackabletech/airflow-operator`.
        pub repo: Url,

        /// The branch to clone; defaults to `main`.
        ///
        /// Since git-sync v4.x.x this field is mapped to the flag `--ref`.
        #[serde(default = "GitSync::default_branch")]
        pub branch: String,

        /// Location in the Git repository containing the resource; defaults to the root folder.
        ///
        /// It can optionally start with `/`, however, no trailing slash is recommended.
        /// An empty string (``) or slash (`/`) corresponds to the root folder in Git.
        #[serde(default = "GitSync::default_git_folder")]
        pub git_folder: PathBuf,

        /// The depth of syncing, i.e. the number of commits to clone; defaults to 1.
        #[serde(default = "GitSync::default_depth")]
        pub depth: u32,

        /// The synchronization interval, e.g. `20s` or `5m`; defaults to `20s`.
        ///
        /// Since git-sync v4.x.x this field is mapped to the flag `--period`.
        #[serde(default = "GitSync::default_wait")]
        pub wait: Duration,

        /// The name of the Secret used to access the repository if it is not public.
        /// This should include two fields: `user` and `password`.
        /// The `password` field can either be an actual password (not recommended) or a GitHub token,
        /// as described [here](https://github.com/kubernetes/git-sync/tree/v4.2.4?tab=readme-ov-file#manual).
        pub credentials_secret: Option<String>,

        /// A map of optional configuration settings that are listed in the git-sync [documentation].
        ///
        /// Also read the git-sync [example] in our documentation.
        ///
        /// [documentation]: https://github.com/kubernetes/git-sync/tree/v4.2.4?tab=readme-ov-file#manual
        /// [example]: DOCS_BASE_URL_PLACEHOLDER/airflow/usage-guide/mounting-dags#_example
        #[serde(default)]
        pub git_sync_conf: BTreeMap<String, String>,
    }
}
