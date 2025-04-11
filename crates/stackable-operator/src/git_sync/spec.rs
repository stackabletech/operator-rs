use std::{collections::BTreeMap, path::PathBuf};

use crate::{time::Duration, utils::COMMON_BASH_TRAP_FUNCTIONS};
use schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

pub const GIT_SYNC_CONTENT: &str = "content-from-git";
pub const GIT_SYNC_SAFE_DIR: &str = "safe.directory";
pub const GIT_SYNC_DIR: &str = "/stackable/app/git";
pub const GIT_SYNC_ROOT: &str = "/tmp/git";
pub const GIT_SYNC_LINK: &str = "current";
pub const GIT_SYNC_NAME: &str = "gitsync";

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitSync {
    /// The git repository URL that will be cloned, for example: `https://github.com/stackabletech/airflow-operator`.
    pub repo: String,

    /// The branch to clone. Defaults to `main`.
    ///
    /// Since git-sync v4.x.x this field is mapped to the flag `--ref`.
    #[serde(default = "GitSync::default_branch")]
    pub branch: String,

    /// The location of the DAG folder, relative to the synced repository root.
    ///
    /// It can optionally start with `/`, however, no trailing slash is recommended.
    /// An empty string (``) or slash (`/`) corresponds to the root folder in Git.
    #[serde(default = "GitSync::default_git_folder")]
    pub git_folder: PathBuf,

    /// The depth of syncing i.e. the number of commits to clone; defaults to 1.
    #[serde(default = "GitSync::default_depth")]
    pub depth: u32,

    /// The synchronization interval, e.g. `20s` or `5m`, defaults to `20s`.
    ///
    /// Since git-sync v4.x.x this field is mapped to the flag `--period`.
    #[serde(default = "GitSync::default_wait")]
    pub wait: Duration,

    /// The name of the Secret used to access the repository if it is not public.
    /// This should include two fields: `user` and `password`.
    /// The `password` field can either be an actual password (not recommended) or a GitHub token,
    /// as described [here](https://github.com/kubernetes/git-sync/tree/v4.2.4?tab=readme-ov-file#manual).
    pub credentials_secret: Option<String>,

    /// A map of optional configuration settings that are listed in the [git-sync documentation](https://github.com/kubernetes/git-sync/tree/v4.2.4?tab=readme-ov-file#manual).
    /// Read the [git sync example](DOCS_BASE_URL_PLACEHOLDER/airflow/usage-guide/mounting-dags#_example).
    #[serde(default)]
    pub git_sync_conf: BTreeMap<String, String>,
}

impl GitSync {
    fn default_branch() -> String {
        "main".to_string()
    }

    fn default_git_folder() -> PathBuf {
        PathBuf::from("/")
    }

    fn default_depth() -> u32 {
        1
    }

    fn default_wait() -> Duration {
        Duration::from_secs(20)
    }

    /// Returns the command arguments for calling git-sync. If git-sync is called when using the
    /// KubernetesExecutor it should only be called once, and from an initContainer; otherwise, the container
    /// is not terminated and the job can not be cleaned up properly (the job/task is created by airflow from
    /// a pod template and is terminated by airflow itself). The `one_time` parameter is used
    /// to indicate this.
    pub fn get_args(&self, one_time: bool) -> Vec<String> {
        let mut git_config = format!("{GIT_SYNC_SAFE_DIR}:{GIT_SYNC_ROOT}");
        let mut git_sync_command = vec![
            "/stackable/git-sync".to_string(),
            format!("--repo={}", self.repo.clone()),
            format!("--ref={}", self.branch),
            format!("--depth={}", self.depth),
            format!("--period={}s", self.wait.as_secs()),
            format!("--link={GIT_SYNC_LINK}"),
            format!("--root={GIT_SYNC_ROOT}"),
        ];
        if !self.git_sync_conf.is_empty() {
            for (key, value) in &self.git_sync_conf {
                // config options that are internal details have
                // constant values and will be ignored here
                if key.eq_ignore_ascii_case("--dest") || key.eq_ignore_ascii_case("--root") {
                    tracing::warn!("Config option {:?} will be ignored...", key);
                } else {
                    // both "-git-config" and "--gitconfig" are recognized by gitsync
                    if key.to_lowercase().ends_with("-git-config") {
                        if value.to_lowercase().contains(GIT_SYNC_SAFE_DIR) {
                            tracing::warn!("Config option {value:?} contains a value for {GIT_SYNC_SAFE_DIR} that overrides
                                the value of this operator. Git-sync functionality will probably not work as expected!");
                        }
                        git_config = format!("{git_config},{value}");
                    } else {
                        git_sync_command.push(format!("{key}={value}"));
                    }
                }
            }
            git_sync_command.push(format!("--git-config='{git_config}'"));
        }

        let mut args: Vec<String> = vec![];

        if one_time {
            // for one-time git-sync calls (which is the case when git-sync runs as an init
            // container in a job created by the KubernetesExecutor), specify this with the relevant
            // parameter and do not push the process into the background
            git_sync_command.push("--one-time=true".to_string());
            args.push(git_sync_command.join(" "));
        } else {
            // otherwise, we need the signal termination code and the process pushed to the background
            git_sync_command.push("&".to_string());
            args.append(&mut vec![
                COMMON_BASH_TRAP_FUNCTIONS.to_string(),
                "prepare_signal_handlers".to_string(),
            ]);
            args.push(git_sync_command.join(" "));
            args.push("wait_for_termination $!".to_string());
        }
        args
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn test_git_sync_defaults() {
        let spec = "
          name: git-sync
          repo: https://github.com/stackabletech/airflow-operator
          ";

        let deserializer = serde_yaml::Deserializer::from_str(spec);
        let git_sync: GitSync =
            serde_yaml::with::singleton_map_recursive::deserialize(deserializer).unwrap();

        // Check values (including defaults)
        assert_eq!(
            git_sync.repo,
            "https://github.com/stackabletech/airflow-operator"
        );
        assert_eq!(git_sync.branch, "main");
        assert_eq!(git_sync.git_folder, PathBuf::from("/"));
        assert_eq!(git_sync.depth, 1);
        assert_eq!(git_sync.wait, Duration::from_secs(20));
        assert_eq!(git_sync.git_sync_conf, BTreeMap::new());

        // Check resulting command
        assert!(git_sync.get_args(false).contains(
          &"/stackable/git-sync --repo=https://github.com/stackabletech/airflow-operator --ref=main --depth=1 --period=20s --link=current --root=/tmp/git &".to_string()
        ));
        assert!(git_sync.get_args(true).contains(
          &"/stackable/git-sync --repo=https://github.com/stackabletech/airflow-operator --ref=main --depth=1 --period=20s --link=current --root=/tmp/git --one-time=true".to_string()
        ));
    }

    #[test]
    fn test_git_sync_config() {
        let spec = "
          name: git-sync
          repo: https://github.com/stackabletech/airflow-operator
          branch: feat/git-sync
          wait: 42h
          gitSyncConf:
            --ref: c63921857618a8c392ad757dda13090fff3d879a
          # Intentionally using trailing slashes here
          gitFolder: ////////tests/templates/kuttl/mount-dags-gitsync/dags
          ";

        let deserializer = serde_yaml::Deserializer::from_str(spec);
        let git_sync: GitSync =
            serde_yaml::with::singleton_map_recursive::deserialize(deserializer).unwrap();

        // Check resulting command
        assert!(git_sync.get_args(false).contains(
          &"/stackable/git-sync --repo=https://github.com/stackabletech/airflow-operator --ref=feat/git-sync --depth=1 --period=151200s --link=current --root=/tmp/git --ref=c63921857618a8c392ad757dda13090fff3d879a --git-config='safe.directory:/tmp/git' &".to_string()
        ));
        assert!(git_sync.get_args(true).contains(
          &"/stackable/git-sync --repo=https://github.com/stackabletech/airflow-operator --ref=feat/git-sync --depth=1 --period=151200s --link=current --root=/tmp/git --ref=c63921857618a8c392ad757dda13090fff3d879a --git-config='safe.directory:/tmp/git' --one-time=true".to_string()
        ));
    }

    #[rstest]
    #[case(
        "\"--git-config\": \"http.sslCAInfo:/tmp/ca-cert/ca.crt\"",
        "--git-config='safe.directory:/tmp/git,http.sslCAInfo:/tmp/ca-cert/ca.crt'"
    )]
    #[case(
        "\"-git-config\": \"http.sslCAInfo:/tmp/ca-cert/ca.crt\"",
        "--git-config='safe.directory:/tmp/git,http.sslCAInfo:/tmp/ca-cert/ca.crt'"
    )]
    #[case(
        "\"--git-config\": http.sslCAInfo:/tmp/ca-cert/ca.crt",
        "--git-config='safe.directory:/tmp/git,http.sslCAInfo:/tmp/ca-cert/ca.crt'"
    )]
    #[case(
        "--git-config: http.sslCAInfo:/tmp/ca-cert/ca.crt",
        "--git-config='safe.directory:/tmp/git,http.sslCAInfo:/tmp/ca-cert/ca.crt'"
    )]
    #[case(
        "'--git-config': 'http.sslCAInfo:/tmp/ca-cert/ca.crt'",
        "--git-config='safe.directory:/tmp/git,http.sslCAInfo:/tmp/ca-cert/ca.crt'"
    )]
    #[case(
        "--git-config: 'http.sslCAInfo:/tmp/ca-cert/ca.crt,safe.directory:/tmp/git2'",
        "--git-config='safe.directory:/tmp/git,http.sslCAInfo:/tmp/ca-cert/ca.crt,safe.directory:/tmp/git2'"
    )]
    fn test_git_sync_git_config(#[case] input: &str, #[case] output: &str) {
        let spec = format!(
            "
          name: git-sync
          repo: https://github.com/stackabletech/airflow-operator
          gitSyncConf:
            {input}
          "
        );

        let deserializer = serde_yaml::Deserializer::from_str(spec.as_str());
        let git_sync: GitSync =
            serde_yaml::with::singleton_map_recursive::deserialize(deserializer).unwrap();

        assert!(git_sync.get_args(false).iter().any(|c| c.contains(output)));
    }
}
