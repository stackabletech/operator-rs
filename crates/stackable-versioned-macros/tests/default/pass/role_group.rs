use std::collections::{HashMap};

use serde::{Deserialize, Serialize};
use stackable_operator::{
    role_utils::{CommonConfiguration, Role},
    schemars::{self, JsonSchema},
    time::Duration,
    commons::affinity::StackableAffinity,
};
use stackable_versioned_macros::versioned;

#[test]
fn main() {
    let role_config = config::v1::SimpleTrinoConfig {
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
}

#[versioned(version(name = "v1alpha1", skip(from)), version(name = "v1"))]
#[derive(serde::Serialize, serde::Deserialize)]
#[derive(Debug)]
struct TrinoClusterSpec {
    // No doc - it's in the struct.
    #[versioned(
        // FIXME: We shouldn't need this
        added(since = "v1alpha1"),
        changed(
            since = "v1",
            from_type = "Option<Role<config::v1::SimpleTrinoConfig>>",
        ),    
    )]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinators: Option<Role<config::v2::SimpleTrinoConfig>>,
}

impl From<v1alpha1::TrinoClusterSpec> for v1::TrinoClusterSpec {
    fn from(v1alpha1: v1alpha1::TrinoClusterSpec) -> Self {
        let coordinators = match v1alpha1.coordinators {
            Some(coordinators) => {
                let Role { config: common_config, role_config, role_groups: _ } = coordinators;
                
                let common_config = CommonConfiguration{
                    config: common_config.config.into(),
                    config_overrides: common_config.config_overrides,
                    env_overrides: common_config.env_overrides,
                    cli_overrides: common_config.cli_overrides,
                    pod_overrides: common_config.pod_overrides
                };

                // FIXME
                let role_groups = HashMap::new();
                Some(Role { config: common_config, role_config, role_groups })
            },
            None => None,
        };

        Self { coordinators }
    }
}

mod config {
    use super::*;

    #[versioned(version(name = "v1", skip(from)), version(name = "v2"))]
    #[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct SimpleTrinoConfig {
        /// Max total memory
        pub query_max_memory: Option<String>,

        /// Max memory per node
        pub query_max_memory_per_node: Option<String>,

        #[serde(default)]
        pub affinity: StackableAffinity,

        /// Graceful shutdown time
        #[versioned(
            // FIXME: We should not need "added" here
            // TODO: docs currently (2024-09-06) only works on top level structs, not fields
            added(
                since = "v1",
                //doc = "Bruh dumme Zahl, e.g. 60 or 180"
            ),
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
        #[serde(default)]
        pub graceful_shutdown_timeout: Option<Duration>,
    }

    impl From<v1::SimpleTrinoConfig> for v2::SimpleTrinoConfig {
        fn from(v1: v1::SimpleTrinoConfig) -> Self {
            let graceful_shutdown_timeout = v1.graceful_shutdown_timeout_seconds.map(|s| Duration::from_secs(s.into()));
            Self {
                query_max_memory: v1.query_max_memory,
                query_max_memory_per_node: v1.query_max_memory_per_node,
                affinity: v1.affinity,
                graceful_shutdown_timeout
            }
        }
    }
}
