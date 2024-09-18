use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use stackable_operator::{
    commons::affinity::StackableAffinity,
    config::{fragment::Fragment, merge::Merge},
    role_utils::{CommonConfiguration, Role},
    schemars::{self, JsonSchema},
    time::Duration,
};
use stackable_versioned_macros::versioned;

fn main() {
    let role_config = config::v1::TrinoConfigFragment {
        query_max_memory: None,
        query_max_memory_per_node: None,
        affinity: Default::default(),
        graceful_shutdown_timeout_seconds: Some(42013000),
    };
    let coordinator_role = Role {
        config: CommonConfiguration {
            config: role_config,
            ..Default::default()
        },
        ..Default::default()
    };
    let v1alpha1 = v1alpha1::TrinoClusterSpec {
        coordinators: Some(coordinator_role),
    };

    dbg!(&v1alpha1);
    let v1: v1::TrinoClusterSpec = v1alpha1.into();
    dbg!(&v1);

    let merged_crd = TrinoCluster::merged_crd("v1").unwrap();
    println!("{}", serde_yaml::to_string(&merged_crd).unwrap());

    // let file = std::fs::File::create("/tmp/trino-crds.yaml").unwrap();
    // serde_yaml::to_writer(file, &merged_crd).unwrap();

    let trino_yaml = r#"
    apiVersion: trino.stackable.tech/v1alpha1
    kind: TrinoCluster
    metadata:
      name: simple-trino
    spec:
      coordinators:
        config:
          gracefulShutdownTimeout: 1h
        roleGroups:
          default:
            replicas: 1
          stirbLangsam:
            config:
              gracefulShutdownTimeout: 131m # https://de.wikipedia.org/wiki/Stirb_langsam
            replicas: 1
    "#;
    let trino: v1::TrinoCluster = serde_yaml::from_str(trino_yaml).expect("illegal test input");

    let role_config = trino.spec.coordinators.clone().unwrap().config.config;

    let get_role_group_config = |role_group_name: &str| {
        trino
            .clone()
            .spec
            .coordinators
            .unwrap()
            .role_groups
            .remove(role_group_name)
            .unwrap()
            .config
            .config
    };

    let mut default_role_group_conf = get_role_group_config("default");
    let mut stirb_langsam_role_group_conf = get_role_group_config("stirbLangsam");

    default_role_group_conf.merge(&role_config);
    stirb_langsam_role_group_conf.merge(&role_config);

    assert_eq!(
        role_config.graceful_shutdown_timeout,
        Some(Duration::from_hours_unchecked(1))
    );
    assert_eq!(
        default_role_group_conf.graceful_shutdown_timeout,
        Some(Duration::from_hours_unchecked(1))
    );
    assert_eq!(
        stirb_langsam_role_group_conf.graceful_shutdown_timeout,
        Some(Duration::from_minutes_unchecked(131))
    );
}

/// A Trino cluster stacklet. This resource is managed by the Stackable operator for Trino.
/// Find more information on how to use it and the resources that the operator generates in the
/// [operator documentation](DOCS_BASE_URL_PLACEHOLDER/trino/).
#[versioned(
    version(name = "v1alpha1", skip(from)),
    version(name = "v1"),
    k8s(group = "trino.stackable.tech")
    // TODO: We need some (!) CRDs to be namespaces
    // k8s(group = "stackable.tech", plural = "trinoclusters", namespaced)
)]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, JsonSchema)]
struct TrinoClusterSpec {
    // No doc - it's in the struct.
    #[versioned(changed(
        since = "v1",
        from_type = "Option<Role<config::v1::TrinoConfigFragment>>",
    ))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinators: Option<Role<config::v2::TrinoConfigFragment>>,
}

impl From<v1alpha1::TrinoClusterSpec> for v1::TrinoClusterSpec {
    fn from(v1alpha1: v1alpha1::TrinoClusterSpec) -> Self {
        let coordinators = match v1alpha1.coordinators {
            Some(coordinators) => {
                let Role {
                    config: common_config,
                    role_config,
                    role_groups: _,
                } = coordinators;

                let common_config = CommonConfiguration {
                    config: common_config.config.into(),
                    config_overrides: common_config.config_overrides,
                    env_overrides: common_config.env_overrides,
                    cli_overrides: common_config.cli_overrides,
                    pod_overrides: common_config.pod_overrides,
                };

                // FIXME
                let role_groups = HashMap::new();
                Some(Role {
                    config: common_config,
                    role_config,
                    role_groups,
                })
            }
            None => None,
        };

        Self { coordinators }
    }
}

mod config {
    use super::*;

    #[versioned(version(name = "v1", skip(from)), version(name = "v2"))]
    #[derive(Clone, Debug, Default, Fragment, JsonSchema, PartialEq)]
    #[fragment_attrs(
        derive(
            Clone,
            Debug,
            Default,
            Deserialize,
            Merge,
            JsonSchema,
            PartialEq,
            Serialize
        ),
        serde(rename_all = "camelCase")
    )]
    pub struct TrinoConfig {
        /// Max total memory
        pub query_max_memory: Option<String>,

        /// Max memory per node
        pub query_max_memory_per_node: Option<String>,

        #[fragment_attrs(serde(default))]
        pub affinity: StackableAffinity,

        /// Graceful shutdown time
        #[versioned(
            // TODO: Add dedicated action for only adding docs:
            // docs(
            //     version = "v1",
            //     doc = "Bruh dumme Zahl, e.g. 60 or 180"
            // )
            changed(
                since = "v2",
                from_name = "graceful_shutdown_timeout_seconds",
                from_type = "Option<u32>",
                // doc = "Time period Pods have to gracefully shut down, e.g. `30m`, `1h` or `2d`. Consult the operator documentation for details."
            ),
        )]
        #[fragment_attrs(serde(default))]
        pub graceful_shutdown_timeout: Option<Duration>,
    }

    impl From<v1::TrinoConfigFragment> for v2::TrinoConfigFragment {
        fn from(v1: v1::TrinoConfigFragment) -> Self {
            let graceful_shutdown_timeout = v1
                .graceful_shutdown_timeout_seconds
                .map(|s| Duration::from_secs(s.into()));
            Self {
                query_max_memory: v1.query_max_memory,
                query_max_memory_per_node: v1.query_max_memory_per_node,
                affinity: v1.affinity,
                graceful_shutdown_timeout,
            }
        }
    }

    impl From<v2::TrinoConfigFragment> for v1::TrinoConfigFragment {
        fn from(v2: v2::TrinoConfigFragment) -> Self {
            let graceful_shutdown_timeout_seconds = v2
                .graceful_shutdown_timeout
                .map(|d| d.as_secs().try_into().expect("Time duration too big :("));
            Self {
                query_max_memory: v2.query_max_memory,
                query_max_memory_per_node: v2.query_max_memory_per_node,
                affinity: v2.affinity,
                graceful_shutdown_timeout_seconds,
            }
        }
    }
}
