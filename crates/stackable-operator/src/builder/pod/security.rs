use k8s_openapi::api::core::v1::{
    Capabilities, PodSecurityContext, SELinuxOptions, SeccompProfile, SecurityContext, Sysctl,
    WindowsSecurityContextOptions,
};

/// A builder for [`SecurityContext`] objects (not to be confused with `PodSecurityContext`).
#[derive(Clone, Default)]
pub struct SecurityContextBuilder {
    security_context: SecurityContext,
}

impl SecurityContextBuilder {
    /// Convenience function for a wide use-case.
    pub fn run_as_root() -> SecurityContext {
        SecurityContext {
            run_as_user: Some(0),
            ..SecurityContext::default()
        }
    }

    pub fn new() -> SecurityContextBuilder {
        SecurityContextBuilder::default()
    }

    pub fn allow_privilege_escalation(&mut self, value: bool) -> &mut Self {
        self.security_context.allow_privilege_escalation = Some(value);
        self
    }

    pub fn capabilities(&mut self, value: Capabilities) -> &mut Self {
        self.security_context.capabilities = Some(value);
        self
    }

    pub fn privileged(&mut self, value: bool) -> &mut Self {
        self.security_context.privileged = Some(value);
        self
    }

    pub fn proc_mount(&mut self, value: impl Into<String>) -> &mut Self {
        self.security_context.proc_mount = Some(value.into());
        self
    }

    pub fn read_only_root_filesystem(&mut self, value: bool) -> &mut Self {
        self.security_context.read_only_root_filesystem = Some(value);
        self
    }

    pub fn run_as_group(&mut self, value: i64) -> &mut Self {
        self.security_context.run_as_group = Some(value);
        self
    }

    pub fn run_as_non_root(&mut self, value: bool) -> &mut Self {
        self.security_context.run_as_non_root = Some(value);
        self
    }

    pub fn run_as_user(&mut self, value: i64) -> &mut Self {
        self.security_context.run_as_user = Some(value);
        self
    }

    pub fn se_linux_level(&mut self, level: impl Into<String>) -> &mut Self {
        let sc = self
            .security_context
            .se_linux_options
            .get_or_insert_with(SELinuxOptions::default);
        sc.level = Some(level.into());
        self
    }
    pub fn se_linux_role(&mut self, role: impl Into<String>) -> &mut Self {
        let sc = self
            .security_context
            .se_linux_options
            .get_or_insert_with(SELinuxOptions::default);
        sc.role = Some(role.into());
        self
    }

    pub fn se_linux_type(&mut self, type_: impl Into<String>) -> &mut Self {
        let sc = self
            .security_context
            .se_linux_options
            .get_or_insert_with(SELinuxOptions::default);
        sc.type_ = Some(type_.into());
        self
    }

    pub fn se_linux_user(&mut self, user: impl Into<String>) -> &mut Self {
        let sc = self
            .security_context
            .se_linux_options
            .get_or_insert_with(SELinuxOptions::default);
        sc.user = Some(user.into());
        self
    }

    pub fn seccomp_profile_localhost(&mut self, profile: impl Into<String>) -> &mut Self {
        let sc = self
            .security_context
            .seccomp_profile
            .get_or_insert_with(SeccompProfile::default);
        sc.localhost_profile = Some(profile.into());
        self
    }

    pub fn seccomp_profile_type(&mut self, type_: impl Into<String>) -> &mut Self {
        let sc = self
            .security_context
            .seccomp_profile
            .get_or_insert_with(SeccompProfile::default);
        sc.type_ = type_.into();
        self
    }

    pub fn win_credential_spec(&mut self, spec: impl Into<String>) -> &mut Self {
        let wo = self
            .security_context
            .windows_options
            .get_or_insert_with(WindowsSecurityContextOptions::default);
        wo.gmsa_credential_spec = Some(spec.into());
        self
    }

    pub fn win_credential_spec_name(&mut self, name: impl Into<String>) -> &mut Self {
        let wo = self
            .security_context
            .windows_options
            .get_or_insert_with(WindowsSecurityContextOptions::default);
        wo.gmsa_credential_spec_name = Some(name.into());
        self
    }

    pub fn win_run_as_user_name(&mut self, name: impl Into<String>) -> &mut Self {
        let wo = self
            .security_context
            .windows_options
            .get_or_insert_with(WindowsSecurityContextOptions::default);
        wo.run_as_user_name = Some(name.into());
        self
    }
}

#[derive(Clone, Default)]
pub struct PodSecurityContextBuilder {
    pod_security_context: PodSecurityContext,
}

impl PodSecurityContextBuilder {
    pub fn new() -> PodSecurityContextBuilder {
        PodSecurityContextBuilder::default()
    }

    pub fn build(&self) -> PodSecurityContext {
        self.pod_security_context.clone()
    }

    pub fn fs_group(&mut self, group: i64) -> &mut Self {
        self.pod_security_context.fs_group = Some(group);
        self
    }

    pub fn fs_group_change_policy(&mut self, policy: &str) -> &mut Self {
        self.pod_security_context.fs_group_change_policy = Some(policy.to_string());
        self
    }

    pub fn run_as_group(&mut self, group: i64) -> &mut Self {
        self.pod_security_context.run_as_group = Some(group);
        self
    }

    pub fn run_as_non_root(&mut self) -> &mut Self {
        self.pod_security_context.run_as_non_root = Some(true);
        self
    }

    pub fn run_as_user(&mut self, user: i64) -> &mut Self {
        self.pod_security_context.run_as_user = Some(user);
        self
    }

    pub fn supplemental_groups(&mut self, groups: &[i64]) -> &mut Self {
        self.pod_security_context.supplemental_groups = Some(groups.to_vec());
        self
    }

    pub fn se_linux_level(&mut self, level: &str) -> &mut Self {
        self.pod_security_context.se_linux_options =
            Some(self.pod_security_context.se_linux_options.clone().map_or(
                SELinuxOptions {
                    level: Some(level.to_string()),
                    ..SELinuxOptions::default()
                },
                |o| SELinuxOptions {
                    level: Some(level.to_string()),
                    ..o
                },
            ));
        self
    }
    pub fn se_linux_role(&mut self, role: &str) -> &mut Self {
        self.pod_security_context.se_linux_options =
            Some(self.pod_security_context.se_linux_options.clone().map_or(
                SELinuxOptions {
                    role: Some(role.to_string()),
                    ..SELinuxOptions::default()
                },
                |o| SELinuxOptions {
                    role: Some(role.to_string()),
                    ..o
                },
            ));
        self
    }
    pub fn se_linux_type(&mut self, type_: &str) -> &mut Self {
        self.pod_security_context.se_linux_options =
            Some(self.pod_security_context.se_linux_options.clone().map_or(
                SELinuxOptions {
                    type_: Some(type_.to_string()),
                    ..SELinuxOptions::default()
                },
                |o| SELinuxOptions {
                    type_: Some(type_.to_string()),
                    ..o
                },
            ));
        self
    }
    pub fn se_linux_user(&mut self, user: &str) -> &mut Self {
        self.pod_security_context.se_linux_options =
            Some(self.pod_security_context.se_linux_options.clone().map_or(
                SELinuxOptions {
                    user: Some(user.to_string()),
                    ..SELinuxOptions::default()
                },
                |o| SELinuxOptions {
                    user: Some(user.to_string()),
                    ..o
                },
            ));
        self
    }

    pub fn seccomp_profile_localhost(&mut self, profile: &str) -> &mut Self {
        self.pod_security_context.seccomp_profile =
            Some(self.pod_security_context.seccomp_profile.clone().map_or(
                SeccompProfile {
                    localhost_profile: Some(profile.to_string()),
                    ..SeccompProfile::default()
                },
                |o| SeccompProfile {
                    localhost_profile: Some(profile.to_string()),
                    ..o
                },
            ));
        self
    }

    pub fn seccomp_profile_type(&mut self, type_: &str) -> &mut Self {
        self.pod_security_context.seccomp_profile =
            Some(self.pod_security_context.seccomp_profile.clone().map_or(
                SeccompProfile {
                    type_: type_.to_string(),
                    ..SeccompProfile::default()
                },
                |o| SeccompProfile {
                    type_: type_.to_string(),
                    ..o
                },
            ));
        self
    }

    pub fn sysctls(&mut self, kparam: &[(&str, &str)]) -> &mut Self {
        self.pod_security_context.sysctls = Some(
            kparam
                .iter()
                .map(|&name_value| Sysctl {
                    name: name_value.0.to_string(),
                    value: name_value.1.to_string(),
                })
                .collect(),
        );
        self
    }

    pub fn win_credential_spec(&mut self, spec: &str) -> &mut Self {
        self.pod_security_context.windows_options =
            Some(self.pod_security_context.windows_options.clone().map_or(
                WindowsSecurityContextOptions {
                    gmsa_credential_spec: Some(spec.to_string()),
                    ..WindowsSecurityContextOptions::default()
                },
                |o| WindowsSecurityContextOptions {
                    gmsa_credential_spec: Some(spec.to_string()),
                    ..o
                },
            ));
        self
    }

    pub fn win_credential_spec_name(&mut self, name: &str) -> &mut Self {
        self.pod_security_context.windows_options =
            Some(self.pod_security_context.windows_options.clone().map_or(
                WindowsSecurityContextOptions {
                    gmsa_credential_spec_name: Some(name.to_string()),
                    ..WindowsSecurityContextOptions::default()
                },
                |o| WindowsSecurityContextOptions {
                    gmsa_credential_spec_name: Some(name.to_string()),
                    ..o
                },
            ));
        self
    }

    pub fn win_run_as_user_name(&mut self, name: &str) -> &mut Self {
        self.pod_security_context.windows_options =
            Some(self.pod_security_context.windows_options.clone().map_or(
                WindowsSecurityContextOptions {
                    run_as_user_name: Some(name.to_string()),
                    ..WindowsSecurityContextOptions::default()
                },
                |o| WindowsSecurityContextOptions {
                    run_as_user_name: Some(name.to_string()),
                    ..o
                },
            ));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{PodSecurityContext, SELinuxOptions, SeccompProfile, Sysctl};

    #[test]
    fn test_security_context_builder() {
        let mut builder = PodSecurityContextBuilder::new();
        let context = builder
            .fs_group(1000)
            .fs_group_change_policy("policy")
            .run_as_user(1001)
            .run_as_group(1001)
            .run_as_non_root()
            .supplemental_groups(&[1002, 1003])
            .se_linux_level("level")
            .se_linux_role("role")
            .se_linux_type("type")
            .se_linux_user("user")
            .seccomp_profile_localhost("localhost")
            .seccomp_profile_type("type")
            .sysctls(&[("param1", "value1"), ("param2", "value2")])
            .win_credential_spec("spec")
            .win_credential_spec_name("name")
            .win_run_as_user_name("winuser")
            .build();

        assert_eq!(
            context,
            PodSecurityContext {
                fs_group: Some(1000),
                fs_group_change_policy: Some("policy".to_string()),
                run_as_user: Some(1001),
                run_as_group: Some(1001),
                run_as_non_root: Some(true),
                supplemental_groups: Some(vec![1002, 1003]),
                se_linux_options: Some(SELinuxOptions {
                    level: Some("level".to_string()),
                    role: Some("role".to_string()),
                    type_: Some("type".to_string()),
                    user: Some("user".to_string()),
                }),
                seccomp_profile: Some(SeccompProfile {
                    localhost_profile: Some("localhost".to_string()),
                    type_: "type".to_string(),
                }),
                sysctls: Some(vec![
                    Sysctl {
                        name: "param1".to_string(),
                        value: "value1".to_string(),
                    },
                    Sysctl {
                        name: "param2".to_string(),
                        value: "value2".to_string(),
                    },
                ]),
                windows_options: Some(WindowsSecurityContextOptions {
                    gmsa_credential_spec: Some("spec".to_string()),
                    gmsa_credential_spec_name: Some("name".to_string()),
                    run_as_user_name: Some("winuser".to_string()),
                    ..Default::default()
                }),
                // This attribute is supported starting with Kubernetes 1.30.
                // Because we support older Kubernetes versions as well, we can
                // not use it for now, as we would not work on older Kubernetes
                // clusters.
                app_armor_profile: None,
                // This attribute is supported starting with Kubernetes 1.31.
                supplemental_groups_policy: None,
            }
        );
    }
}
