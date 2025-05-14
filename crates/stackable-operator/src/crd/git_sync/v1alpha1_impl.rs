use std::{collections::BTreeMap, path::PathBuf};

use k8s_openapi::api::core::v1::{
    Container, EmptyDirVolumeSource, EnvVar, EnvVarSource, SecretKeySelector, Volume, VolumeMount,
};
use snafu::{ResultExt, Snafu};
use strum::{EnumDiscriminants, IntoStaticStr};

use crate::{
    builder::pod::{
        container::ContainerBuilder, resources::ResourceRequirementsBuilder, volume::VolumeBuilder,
    },
    commons::product_image_selection::ResolvedProductImage,
    crd::git_sync::v1alpha1::GitSync,
    product_config_utils::insert_or_update_env_vars,
    product_logging::{
        framework::capture_shell_output,
        spec::{ContainerLogConfig, ContainerLogConfigChoice},
    },
    time::Duration,
    utils::COMMON_BASH_TRAP_FUNCTIONS,
};

pub const CONTAINER_NAME_PREFIX: &str = "git-sync";
pub const VOLUME_NAME_PREFIX: &str = "content-from-git";
pub const MOUNT_PATH_PREFIX: &str = "/stackable/app/git";
pub const GIT_SYNC_SAFE_DIR_OPTION: &str = "safe.directory";
pub const GIT_SYNC_ROOT_DIR: &str = "/tmp/git";
pub const GIT_SYNC_LINK: &str = "current";

#[derive(Snafu, Debug, EnumDiscriminants)]
#[strum_discriminants(derive(IntoStaticStr))]
pub enum Error {
    #[snafu(display("invalid container name"))]
    InvalidContainerName {
        source: crate::builder::pod::container::Error,
    },

    #[snafu(display("failed to add needed volumeMount"))]
    AddVolumeMount {
        source: crate::builder::pod::container::Error,
    },
}

impl GitSync {
    pub(crate) fn default_branch() -> String {
        "main".to_string()
    }

    pub(crate) fn default_git_folder() -> PathBuf {
        PathBuf::from("/")
    }

    pub(crate) fn default_depth() -> u32 {
        1
    }

    pub(crate) fn default_wait() -> Duration {
        Duration::from_secs(20)
    }
}

/// Kubernetes resources generated from `GitSync` specifications which should be added to the Pod.
#[derive(Default)]
pub struct GitSyncResources {
    /// GitSync containers with regular synchronizations
    pub git_sync_containers: Vec<Container>,
    /// GitSync init containers with a one-time synchronizations
    pub git_sync_init_containers: Vec<Container>,
    /// GitSync volumes containing the synchronized repository
    pub git_content_volumes: Vec<Volume>,
    /// Volume mounts for the GitSync volumes
    pub git_content_volume_mounts: Vec<VolumeMount>,
    /// Absolute paths to the Git contents in the mounted volumes
    pub git_content_folders: Vec<PathBuf>,
}

impl GitSyncResources {
    const LOG_VOLUME_MOUNT_PATH: &str = "/stackable/log";

    /// Returns whether or not GitSync is enabled.
    pub fn is_git_sync_enabled(&self) -> bool {
        !self.git_sync_containers.is_empty()
    }

    /// Returns the Git content folders as strings
    pub fn git_content_folders_as_string(&self) -> Vec<String> {
        self.git_content_folders
            .iter()
            .map(|path| path.to_str().expect("The path names of the git_content_folders are created as valid UTF-8 strings, so Path::to_str should not fail.").to_string())
            .collect()
    }

    /// Creates `GitSyncResources` from the given `GitSync` specifications.
    pub fn new(
        git_syncs: &[GitSync],
        resolved_product_image: &ResolvedProductImage,
        extra_env_vars: &[EnvVar],
        extra_volume_mounts: &[VolumeMount],
        log_volume_name: &str,
        container_log_config: &ContainerLogConfig,
    ) -> Result<GitSyncResources, Error> {
        let mut resources = GitSyncResources::default();

        for (i, git_sync) in git_syncs.iter().enumerate() {
            let mut env_vars = vec![];
            if let Some(git_credentials_secret) = &git_sync.credentials_secret {
                env_vars.push(GitSyncResources::env_var_from_secret(
                    "GITSYNC_USERNAME",
                    git_credentials_secret,
                    "user",
                ));
                env_vars.push(GitSyncResources::env_var_from_secret(
                    "GITSYNC_PASSWORD",
                    git_credentials_secret,
                    "password",
                ));
            }
            env_vars = insert_or_update_env_vars(&env_vars, extra_env_vars);

            let volume_name = format!("{VOLUME_NAME_PREFIX}-{i}");
            let mount_path = format!("{MOUNT_PATH_PREFIX}-{i}");

            let git_sync_root_volume_mount = VolumeMount {
                name: volume_name.to_owned(),
                mount_path: GIT_SYNC_ROOT_DIR.to_string(),
                ..VolumeMount::default()
            };

            let log_volume_mount = VolumeMount {
                name: log_volume_name.to_string(),
                mount_path: Self::LOG_VOLUME_MOUNT_PATH.to_string(),
                ..VolumeMount::default()
            };

            let mut git_sync_container_volume_mounts =
                vec![git_sync_root_volume_mount, log_volume_mount];
            git_sync_container_volume_mounts.extend_from_slice(extra_volume_mounts);

            let container = Self::create_git_sync_container(
                &format!("{CONTAINER_NAME_PREFIX}-{i}"),
                resolved_product_image,
                git_sync,
                false,
                &env_vars,
                &git_sync_container_volume_mounts,
                container_log_config,
            )?;

            let init_container = Self::create_git_sync_container(
                &format!("{CONTAINER_NAME_PREFIX}-{i}-init"),
                resolved_product_image,
                git_sync,
                true,
                &env_vars,
                &git_sync_container_volume_mounts,
                container_log_config,
            )?;

            let volume = VolumeBuilder::new(volume_name.to_owned())
                .empty_dir(EmptyDirVolumeSource::default())
                .build();

            let git_content_volume_mount = VolumeMount {
                name: volume_name.to_owned(),
                mount_path: mount_path.to_owned(),
                ..VolumeMount::default()
            };

            let mut git_content_folder = PathBuf::from(mount_path);
            let relative_git_folder = git_sync
                .git_folder
                .strip_prefix("/")
                .unwrap_or(&git_sync.git_folder);
            git_content_folder.push(GIT_SYNC_LINK);
            git_content_folder.push(relative_git_folder);

            resources.git_sync_containers.push(container);
            resources.git_sync_init_containers.push(init_container);
            resources.git_content_volumes.push(volume);
            resources
                .git_content_volume_mounts
                .push(git_content_volume_mount);
            resources.git_content_folders.push(git_content_folder);
        }

        Ok(resources)
    }

    fn create_git_sync_container(
        container_name: &str,
        resolved_product_image: &ResolvedProductImage,
        git_sync: &GitSync,
        one_time: bool,
        env_vars: &[EnvVar],
        volume_mounts: &[VolumeMount],
        container_log_config: &ContainerLogConfig,
    ) -> Result<k8s_openapi::api::core::v1::Container, Error> {
        let container = ContainerBuilder::new(container_name)
            .context(InvalidContainerNameSnafu)?
            .image_from_product_image(resolved_product_image)
            .command(vec![
                "/bin/bash".to_string(),
                "-x".to_string(),
                "-euo".to_string(),
                "pipefail".to_string(),
                "-c".to_string(),
            ])
            .args(vec![Self::create_git_sync_shell_script(
                container_name,
                git_sync,
                one_time,
                container_log_config,
            )])
            .add_env_vars(env_vars.into())
            .add_volume_mounts(volume_mounts.to_vec())
            .context(AddVolumeMountSnafu)?
            .resources(
                ResourceRequirementsBuilder::new()
                    .with_cpu_request("100m")
                    .with_cpu_limit("200m")
                    .with_memory_request("64Mi")
                    .with_memory_limit("64Mi")
                    .build(),
            )
            .build();
        Ok(container)
    }

    fn create_git_sync_shell_script(
        container_name: &str,
        git_sync: &GitSync,
        one_time: bool,
        container_log_config: &ContainerLogConfig,
    ) -> String {
        let internal_args = [
            Some(("--repo".to_string(), git_sync.repo.as_str().to_owned())),
            Some(("--ref".to_string(), git_sync.branch.to_owned())),
            Some(("--depth".to_string(), git_sync.depth.to_string())),
            Some((
                "--period".to_string(),
                format!("{}s", git_sync.wait.as_secs()),
            )),
            Some(("--link".to_string(), GIT_SYNC_LINK.to_string())),
            Some(("--root".to_string(), GIT_SYNC_ROOT_DIR.to_string())),
            one_time.then_some(("--one-time".to_string(), "true".to_string())),
        ]
        .into_iter()
        .flatten()
        .collect::<BTreeMap<_, _>>();

        let internal_git_config = [(
            GIT_SYNC_SAFE_DIR_OPTION.to_string(),
            GIT_SYNC_ROOT_DIR.to_string(),
        )]
        .into_iter()
        .collect::<BTreeMap<_, _>>();

        let mut user_defined_args = BTreeMap::new();
        // The key and value in Git configs are separated by a colon, but both
        // can contain either escaped colons or unescaped colons if enclosed in
        // quotes. To avoid parsing, just a vector is used instead of a map.
        let mut user_defined_git_configs = Vec::new();

        for (key, value) in &git_sync.git_sync_conf {
            // The initial git-sync implementation in the airflow-operator
            // (https://github.com/stackabletech/airflow-operator/pull/381)
            // used this condition to find Git configs. It is also used here
            // for backwards-compatibility:
            if key.to_lowercase().ends_with("-git-config") {
                // Roughly check if the user defined config contains an
                // internally defined config and emit a warning in case.
                if internal_git_config.keys().any(|key| value.contains(key)) {
                    tracing::warn!("Config option {value:?} contains a value for {GIT_SYNC_SAFE_DIR_OPTION} that overrides
                                the value of this operator. Git-sync functionality will probably not work as expected!");
                }
                user_defined_git_configs.push(value.to_owned());
            } else if internal_args.contains_key(key) {
                tracing::warn!(
                    "The git-sync option {key:?} is already internally defined and will be ignored."
                );
            } else {
                // The user-defined arguments are not validated.
                user_defined_args.insert(key.to_owned(), value.to_owned());
            }
        }

        // The user-defined Git config is just appended.
        // The user is responsible for escaping special characters like `:` and `,`.
        let git_config = internal_git_config
            .into_iter()
            .map(|(key, value)| format!("{key}:{value}"))
            .chain(user_defined_git_configs)
            .collect::<Vec<_>>()
            .join(",");

        let mut args = internal_args;
        args.extend(user_defined_args);
        args.insert("--git-config".to_string(), format!("'{git_config}'"));

        let args_string = args
            .into_iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(" ");

        let mut shell_script = String::new();

        if let ContainerLogConfig {
            choice: Some(ContainerLogConfigChoice::Automatic(log_config)),
        } = container_log_config
        {
            shell_script.push_str(&capture_shell_output(
                Self::LOG_VOLUME_MOUNT_PATH,
                container_name,
                log_config,
            ));
            shell_script.push('\n');
        };

        let git_sync_command = format!("/stackable/git-sync {args_string}");

        if one_time {
            shell_script.push_str(&git_sync_command);
        } else {
            // Run the git-sync command in the background
            shell_script.push_str(&format!(
                "{COMMON_BASH_TRAP_FUNCTIONS}
prepare_signal_handlers
{git_sync_command} &
wait_for_termination $!"
            ))
        }

        shell_script
    }

    fn env_var_from_secret(var_name: &str, secret: &str, secret_key: &str) -> EnvVar {
        EnvVar {
            name: var_name.to_string(),
            value_from: Some(EnvVarSource {
                secret_key_ref: Some(SecretKeySelector {
                    name: secret.to_string(),
                    key: secret_key.to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::fragment::validate, product_config_utils::env_vars_from,
        product_logging::spec::default_container_log_config,
    };

    #[test]
    fn test_no_git_sync() {
        let git_syncs = [];

        let resolved_product_image = ResolvedProductImage {
            image: "oci.stackable.tech/sdp/product:latest".to_string(),
            app_version_label: "1.0.0-latest".to_string(),
            product_version: "1.0.0".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: None,
        };

        let extra_env_vars = [];

        let extra_volume_mounts = [];

        let git_sync_resources = GitSyncResources::new(
            &git_syncs,
            &resolved_product_image,
            &extra_env_vars,
            &extra_volume_mounts,
            "log-volume",
            &validate(default_container_log_config()).unwrap(),
        )
        .unwrap();

        assert!(!git_sync_resources.is_git_sync_enabled());
        assert!(git_sync_resources.git_sync_containers.is_empty());
        assert!(git_sync_resources.git_sync_init_containers.is_empty());
        assert!(git_sync_resources.git_content_volumes.is_empty());
        assert!(git_sync_resources.git_content_volume_mounts.is_empty());
        assert!(git_sync_resources.git_content_folders.is_empty());
    }

    #[test]
    fn test_multiple_git_syncs() {
        let git_sync_spec = r#"
          # GitSync with defaults
          - repo: https://github.com/stackabletech/repo1

          # GitSync with usual configuration
          - repo: https://github.com/stackabletech/repo2
            branch: trunk
            gitFolder: ""
            depth: 3
            wait: 1m
            credentialsSecret: git-credentials
            gitSyncConf:
              --rev: HEAD
              --git-config: http.sslCAInfo:/tmp/ca-cert/ca.crt

          # GitSync with unusual configuration
          - repo: https://github.com/stackabletech/repo3
            branch: feat/git-sync
            # leading slashes should be removed
            gitFolder: ////folder
            gitSyncConf:
              --depth: internal option which should be ignored
              --link: internal option which should be ignored
              --period: internal option which should be ignored
              --ref: internal option which should be ignored
              --repo: internal option which should be ignored
              --root: internal option which should be ignored
              --GIT-CONFIG: k1:v1
              # safe.directory should be accepted but a warning will be emitted
              --git-config: k2:v2,safe.directory:/safe-dir
              -GIT-CONFIG: k3:v3
              -git-config: k4:v4
          "#;

        let deserializer = serde_yaml::Deserializer::from_str(git_sync_spec);
        let git_syncs: Vec<GitSync> =
            serde_yaml::with::singleton_map_recursive::deserialize(deserializer).unwrap();

        let resolved_product_image = ResolvedProductImage {
            image: "oci.stackable.tech/sdp/product:latest".to_string(),
            app_version_label: "1.0.0-latest".to_string(),
            product_version: "1.0.0".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: None,
        };

        let extra_env_vars = env_vars_from([
            ("VAR1", "value1"),
            ("GITSYNC_USERNAME", "overriden-username"),
        ]);

        let extra_volume_mounts = [VolumeMount {
            name: "extra-volume".to_string(),
            mount_path: "/mnt/extra-volume".to_string(),
            ..VolumeMount::default()
        }];

        let git_sync_resources = GitSyncResources::new(
            &git_syncs,
            &resolved_product_image,
            &extra_env_vars,
            &extra_volume_mounts,
            "log-volume",
            &validate(default_container_log_config()).unwrap(),
        )
        .unwrap();

        assert!(git_sync_resources.is_git_sync_enabled());

        assert_eq!(3, git_sync_resources.git_sync_containers.len());

        assert_eq!(
            r#"args:
- |-
  mkdir --parents /stackable/log/git-sync-0 && exec > >(tee /stackable/log/git-sync-0/container.stdout.log) 2> >(tee /stackable/log/git-sync-0/container.stderr.log >&2)

  prepare_signal_handlers()
  {
      unset term_child_pid
      unset term_kill_needed
      trap 'handle_term_signal' TERM
  }

  handle_term_signal()
  {
      if [ "${term_child_pid}" ]; then
          kill -TERM "${term_child_pid}" 2>/dev/null
      else
          term_kill_needed="yes"
      fi
  }

  wait_for_termination()
  {
      set +e
      term_child_pid=$1
      if [[ -v term_kill_needed ]]; then
          kill -TERM "${term_child_pid}" 2>/dev/null
      fi
      wait ${term_child_pid} 2>/dev/null
      trap - TERM
      wait ${term_child_pid} 2>/dev/null
      set -e
  }

  prepare_signal_handlers
  /stackable/git-sync --depth=1 --git-config='safe.directory:/tmp/git' --link=current --period=20s --ref=main --repo=https://github.com/stackabletech/repo1 --root=/tmp/git &
  wait_for_termination $!
command:
- /bin/bash
- -x
- -euo
- pipefail
- -c
env:
- name: GITSYNC_USERNAME
  value: overriden-username
- name: VAR1
  value: value1
image: oci.stackable.tech/sdp/product:latest
imagePullPolicy: Always
name: git-sync-0
resources:
  limits:
    cpu: 200m
    memory: 64Mi
  requests:
    cpu: 100m
    memory: 64Mi
volumeMounts:
- mountPath: /tmp/git
  name: content-from-git-0
- mountPath: /stackable/log
  name: log-volume
- mountPath: /mnt/extra-volume
  name: extra-volume
"#,
            serde_yaml::to_string(&git_sync_resources.git_sync_containers.first()).unwrap()
        );

        assert_eq!(
            r#"args:
- |-
  mkdir --parents /stackable/log/git-sync-1 && exec > >(tee /stackable/log/git-sync-1/container.stdout.log) 2> >(tee /stackable/log/git-sync-1/container.stderr.log >&2)

  prepare_signal_handlers()
  {
      unset term_child_pid
      unset term_kill_needed
      trap 'handle_term_signal' TERM
  }

  handle_term_signal()
  {
      if [ "${term_child_pid}" ]; then
          kill -TERM "${term_child_pid}" 2>/dev/null
      else
          term_kill_needed="yes"
      fi
  }

  wait_for_termination()
  {
      set +e
      term_child_pid=$1
      if [[ -v term_kill_needed ]]; then
          kill -TERM "${term_child_pid}" 2>/dev/null
      fi
      wait ${term_child_pid} 2>/dev/null
      trap - TERM
      wait ${term_child_pid} 2>/dev/null
      set -e
  }

  prepare_signal_handlers
  /stackable/git-sync --depth=3 --git-config='safe.directory:/tmp/git,http.sslCAInfo:/tmp/ca-cert/ca.crt' --link=current --period=60s --ref=trunk --repo=https://github.com/stackabletech/repo2 --rev=HEAD --root=/tmp/git &
  wait_for_termination $!
command:
- /bin/bash
- -x
- -euo
- pipefail
- -c
env:
- name: GITSYNC_PASSWORD
  valueFrom:
    secretKeyRef:
      key: password
      name: git-credentials
- name: GITSYNC_USERNAME
  value: overriden-username
- name: VAR1
  value: value1
image: oci.stackable.tech/sdp/product:latest
imagePullPolicy: Always
name: git-sync-1
resources:
  limits:
    cpu: 200m
    memory: 64Mi
  requests:
    cpu: 100m
    memory: 64Mi
volumeMounts:
- mountPath: /tmp/git
  name: content-from-git-1
- mountPath: /stackable/log
  name: log-volume
- mountPath: /mnt/extra-volume
  name: extra-volume
"#,
            serde_yaml::to_string(&git_sync_resources.git_sync_containers.get(1)).unwrap()
        );

        assert_eq!(
            r#"args:
- |-
  mkdir --parents /stackable/log/git-sync-2 && exec > >(tee /stackable/log/git-sync-2/container.stdout.log) 2> >(tee /stackable/log/git-sync-2/container.stderr.log >&2)

  prepare_signal_handlers()
  {
      unset term_child_pid
      unset term_kill_needed
      trap 'handle_term_signal' TERM
  }

  handle_term_signal()
  {
      if [ "${term_child_pid}" ]; then
          kill -TERM "${term_child_pid}" 2>/dev/null
      else
          term_kill_needed="yes"
      fi
  }

  wait_for_termination()
  {
      set +e
      term_child_pid=$1
      if [[ -v term_kill_needed ]]; then
          kill -TERM "${term_child_pid}" 2>/dev/null
      fi
      wait ${term_child_pid} 2>/dev/null
      trap - TERM
      wait ${term_child_pid} 2>/dev/null
      set -e
  }

  prepare_signal_handlers
  /stackable/git-sync --depth=1 --git-config='safe.directory:/tmp/git,k1:v1,k2:v2,safe.directory:/safe-dir,k3:v3,k4:v4' --link=current --period=20s --ref=feat/git-sync --repo=https://github.com/stackabletech/repo3 --root=/tmp/git &
  wait_for_termination $!
command:
- /bin/bash
- -x
- -euo
- pipefail
- -c
env:
- name: GITSYNC_USERNAME
  value: overriden-username
- name: VAR1
  value: value1
image: oci.stackable.tech/sdp/product:latest
imagePullPolicy: Always
name: git-sync-2
resources:
  limits:
    cpu: 200m
    memory: 64Mi
  requests:
    cpu: 100m
    memory: 64Mi
volumeMounts:
- mountPath: /tmp/git
  name: content-from-git-2
- mountPath: /stackable/log
  name: log-volume
- mountPath: /mnt/extra-volume
  name: extra-volume
"#,
            serde_yaml::to_string(&git_sync_resources.git_sync_containers.get(2)).unwrap()
        );

        assert_eq!(3, git_sync_resources.git_sync_init_containers.len());

        assert_eq!(
            r#"args:
- |-
  mkdir --parents /stackable/log/git-sync-0-init && exec > >(tee /stackable/log/git-sync-0-init/container.stdout.log) 2> >(tee /stackable/log/git-sync-0-init/container.stderr.log >&2)
  /stackable/git-sync --depth=1 --git-config='safe.directory:/tmp/git' --link=current --one-time=true --period=20s --ref=main --repo=https://github.com/stackabletech/repo1 --root=/tmp/git
command:
- /bin/bash
- -x
- -euo
- pipefail
- -c
env:
- name: GITSYNC_USERNAME
  value: overriden-username
- name: VAR1
  value: value1
image: oci.stackable.tech/sdp/product:latest
imagePullPolicy: Always
name: git-sync-0-init
resources:
  limits:
    cpu: 200m
    memory: 64Mi
  requests:
    cpu: 100m
    memory: 64Mi
volumeMounts:
- mountPath: /tmp/git
  name: content-from-git-0
- mountPath: /stackable/log
  name: log-volume
- mountPath: /mnt/extra-volume
  name: extra-volume
"#,
            serde_yaml::to_string(&git_sync_resources.git_sync_init_containers.first()).unwrap()
        );

        assert_eq!(
            r#"args:
- |-
  mkdir --parents /stackable/log/git-sync-1-init && exec > >(tee /stackable/log/git-sync-1-init/container.stdout.log) 2> >(tee /stackable/log/git-sync-1-init/container.stderr.log >&2)
  /stackable/git-sync --depth=3 --git-config='safe.directory:/tmp/git,http.sslCAInfo:/tmp/ca-cert/ca.crt' --link=current --one-time=true --period=60s --ref=trunk --repo=https://github.com/stackabletech/repo2 --rev=HEAD --root=/tmp/git
command:
- /bin/bash
- -x
- -euo
- pipefail
- -c
env:
- name: GITSYNC_PASSWORD
  valueFrom:
    secretKeyRef:
      key: password
      name: git-credentials
- name: GITSYNC_USERNAME
  value: overriden-username
- name: VAR1
  value: value1
image: oci.stackable.tech/sdp/product:latest
imagePullPolicy: Always
name: git-sync-1-init
resources:
  limits:
    cpu: 200m
    memory: 64Mi
  requests:
    cpu: 100m
    memory: 64Mi
volumeMounts:
- mountPath: /tmp/git
  name: content-from-git-1
- mountPath: /stackable/log
  name: log-volume
- mountPath: /mnt/extra-volume
  name: extra-volume
"#,
            serde_yaml::to_string(&git_sync_resources.git_sync_init_containers.get(1)).unwrap()
        );

        assert_eq!(
            r#"args:
- |-
  mkdir --parents /stackable/log/git-sync-2-init && exec > >(tee /stackable/log/git-sync-2-init/container.stdout.log) 2> >(tee /stackable/log/git-sync-2-init/container.stderr.log >&2)
  /stackable/git-sync --depth=1 --git-config='safe.directory:/tmp/git,k1:v1,k2:v2,safe.directory:/safe-dir,k3:v3,k4:v4' --link=current --one-time=true --period=20s --ref=feat/git-sync --repo=https://github.com/stackabletech/repo3 --root=/tmp/git
command:
- /bin/bash
- -x
- -euo
- pipefail
- -c
env:
- name: GITSYNC_USERNAME
  value: overriden-username
- name: VAR1
  value: value1
image: oci.stackable.tech/sdp/product:latest
imagePullPolicy: Always
name: git-sync-2-init
resources:
  limits:
    cpu: 200m
    memory: 64Mi
  requests:
    cpu: 100m
    memory: 64Mi
volumeMounts:
- mountPath: /tmp/git
  name: content-from-git-2
- mountPath: /stackable/log
  name: log-volume
- mountPath: /mnt/extra-volume
  name: extra-volume
"#,
            serde_yaml::to_string(&git_sync_resources.git_sync_init_containers.get(2)).unwrap()
        );

        assert_eq!(3, git_sync_resources.git_content_volumes.len());

        assert_eq!(
            "emptyDir: {}
name: content-from-git-0
",
            serde_yaml::to_string(&git_sync_resources.git_content_volumes.first()).unwrap()
        );

        assert_eq!(
            "emptyDir: {}
name: content-from-git-1
",
            serde_yaml::to_string(&git_sync_resources.git_content_volumes.get(1)).unwrap()
        );

        assert_eq!(
            "emptyDir: {}
name: content-from-git-2
",
            serde_yaml::to_string(&git_sync_resources.git_content_volumes.get(2)).unwrap()
        );

        assert_eq!(3, git_sync_resources.git_content_volume_mounts.len());

        assert_eq!(
            "mountPath: /stackable/app/git-0
name: content-from-git-0
",
            serde_yaml::to_string(&git_sync_resources.git_content_volume_mounts.first()).unwrap()
        );

        assert_eq!(
            "mountPath: /stackable/app/git-1
name: content-from-git-1
",
            serde_yaml::to_string(&git_sync_resources.git_content_volume_mounts.get(1)).unwrap()
        );

        assert_eq!(
            "mountPath: /stackable/app/git-2
name: content-from-git-2
",
            serde_yaml::to_string(&git_sync_resources.git_content_volume_mounts.get(2)).unwrap()
        );

        assert_eq!(3, git_sync_resources.git_content_folders.len());

        assert_eq!(
            "/stackable/app/git-0/current/",
            git_sync_resources
                .git_content_folders_as_string()
                .first()
                .unwrap()
        );

        assert_eq!(
            "/stackable/app/git-1/current/",
            git_sync_resources
                .git_content_folders_as_string()
                .get(1)
                .unwrap()
        );

        assert_eq!(
            "/stackable/app/git-2/current/folder",
            git_sync_resources
                .git_content_folders_as_string()
                .get(2)
                .unwrap()
        );
    }
}
