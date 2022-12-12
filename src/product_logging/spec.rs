//! Logging structure used within Custom Resource Definitions

use std::collections::BTreeMap;
use std::fmt::Display;

use crate::config::fragment::{self, FromFragment};
use crate::config::merge::Atomic;
use crate::config::{fragment::Fragment, merge::Merge};

use derivative::Derivative;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Logging configuration
///
/// The type parameter `T` should be an enum listing all containers:
///
/// ```
/// use serde::{Deserialize, Serialize};
/// use stackable_operator::{
///     product_logging,
///     schemars::JsonSchema,
/// };
/// use strum::{Display, EnumIter};
///
/// #[derive(
///     Clone,
///     Debug,
///     Deserialize,
///     Display,
///     Eq,
///     EnumIter,
///     JsonSchema,
///     Ord,
///     PartialEq,
///     PartialOrd,
///     Serialize,
/// )]
/// #[serde(rename_all = "camelCase")]
/// pub enum Container {
///     Init,
///     Product,
///     Vector,
/// }
///
/// let logging = product_logging::spec::default_logging::<Container>();
/// ```
#[derive(Clone, Debug, Derivative, Eq, Fragment, JsonSchema, PartialEq)]
#[derivative(Default(bound = ""))]
#[fragment(path_overrides(fragment = "crate::config::fragment"))]
#[fragment_attrs(
    derive(
        Clone,
        Debug,
        Derivative,
        Deserialize,
        JsonSchema,
        Merge,
        PartialEq,
        Serialize
    ),
    derivative(Default(bound = "")),
    merge(path_overrides(merge = "crate::config::merge")),
    serde(
        bound(serialize = "T: Serialize", deserialize = "T: Deserialize<'de>",),
        rename_all = "camelCase",
    )
)]
pub struct Logging<T>
where
    T: Clone + Display + Ord,
{
    /// Wether or not to deploy a container with the Vector log agent
    pub enable_vector_agent: bool,
    /// Log configuration per container
    #[fragment_attrs(serde(default))]
    pub containers: BTreeMap<T, ContainerLogConfig>,
}

/// Log configuration of the container
#[derive(Clone, Debug, Default, Eq, Fragment, JsonSchema, PartialEq)]
#[fragment(path_overrides(fragment = "crate::config::fragment"))]
#[fragment_attrs(
    derive(
        Clone,
        Debug,
        Default,
        Deserialize,
        JsonSchema,
        Merge,
        PartialEq,
        Serialize
    ),
    merge(path_overrides(merge = "crate::config::merge")),
    serde(rename_all = "camelCase")
)]
pub struct ContainerLogConfig {
    /// Custom or automatic log configuration
    #[fragment_attrs(serde(flatten))]
    pub choice: Option<ContainerLogConfigChoice>,
}

/// Custom or automatic log configuration
///
/// The custom log configuration takes precedence over the automatic one.
#[derive(Clone, Debug, Derivative, Eq, JsonSchema, PartialEq)]
#[derivative(Default)]
pub enum ContainerLogConfigChoice {
    /// Custom log configuration provided in a ConfigMap
    Custom(CustomContainerLogConfig),
    /// Automatic log configuration according to the given values
    #[derivative(Default)]
    Automatic(AutomaticContainerLogConfig),
}

/// Fragment derived from `ContainerLogConfigChoice`
#[derive(Clone, Debug, Derivative, Deserialize, JsonSchema, Merge, PartialEq, Serialize)]
#[derivative(Default)]
#[merge(path_overrides(merge = "crate::config::merge"))]
#[serde(untagged)]
pub enum ContainerLogConfigChoiceFragment {
    /// Custom log configuration provided in a ConfigMap
    Custom(CustomContainerLogConfigFragment),
    #[derivative(Default)]
    /// Automatic log configuration according to the given values
    Automatic(AutomaticContainerLogConfigFragment),
}

impl FromFragment for ContainerLogConfigChoice {
    type Fragment = ContainerLogConfigChoiceFragment;
    type RequiredFragment = ContainerLogConfigChoiceFragment;

    fn from_fragment(
        fragment: Self::Fragment,
        validator: fragment::Validator,
    ) -> Result<Self, fragment::ValidationError> {
        match fragment {
            Self::Fragment::Custom(fragment) => Ok(Self::Custom(FromFragment::from_fragment(
                fragment, validator,
            )?)),
            Self::Fragment::Automatic(fragment) => Ok(Self::Automatic(
                FromFragment::from_fragment(fragment, validator)?,
            )),
        }
    }
}

/// Log configuration for a container provided in a ConfigMap
#[derive(Clone, Debug, Default, Eq, Fragment, JsonSchema, PartialEq)]
#[fragment(path_overrides(fragment = "crate::config::fragment"))]
#[fragment_attrs(
    derive(
        Clone,
        Debug,
        Default,
        Deserialize,
        JsonSchema,
        Merge,
        PartialEq,
        Serialize
    ),
    merge(path_overrides(merge = "crate::config::merge")),
    serde(rename_all = "camelCase")
)]
pub struct CustomContainerLogConfig {
    pub custom: ConfigMapLogConfig,
}

/// Log configuration provided in a ConfigMap
#[derive(Clone, Debug, Default, Eq, Fragment, JsonSchema, PartialEq)]
#[fragment(path_overrides(fragment = "crate::config::fragment"))]
#[fragment_attrs(
    derive(
        Clone,
        Debug,
        Default,
        Deserialize,
        JsonSchema,
        Merge,
        PartialEq,
        Serialize
    ),
    merge(path_overrides(merge = "crate::config::merge")),
    serde(rename_all = "camelCase")
)]
pub struct ConfigMapLogConfig {
    /// ConfigMap containing the log configuration files
    #[fragment_attrs(serde(default))]
    pub config_map: String,
}

/// Generic log configuration
#[derive(Clone, Debug, Default, Eq, Fragment, JsonSchema, PartialEq)]
#[fragment(path_overrides(fragment = "crate::config::fragment"))]
#[fragment_attrs(
    derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize),
    serde(rename_all = "camelCase")
)]
pub struct AutomaticContainerLogConfig {
    /// Configuration per logger
    #[fragment_attrs(serde(default))]
    pub loggers: BTreeMap<String, LoggerConfig>,
    /// Configuration for the console appender
    pub console: Option<AppenderConfig>,
    /// Configuration for the file appender
    pub file: Option<AppenderConfig>,
}

impl Merge for AutomaticContainerLogConfigFragment {
    fn merge(&mut self, defaults: &Self) {
        self.loggers.merge(&defaults.loggers);
        if let Some(console) = &mut self.console {
            if let Some(defaults_console) = &defaults.console {
                console.merge(defaults_console);
            }
        } else {
            self.console = defaults.console.clone();
        }
        if let Some(file) = &mut self.file {
            if let Some(defaults_file) = &defaults.file {
                file.merge(defaults_file);
            }
        } else {
            self.file = defaults.file.clone();
        }
    }
}

impl AutomaticContainerLogConfig {
    /// Name of the root logger
    pub const ROOT_LOGGER: &'static str = "ROOT";

    /// Return the log level of the root logger
    pub fn root_log_level(&self) -> LogLevel {
        self.loggers
            .get(Self::ROOT_LOGGER)
            .map(|root| root.level.to_owned())
            .unwrap_or_default()
    }
}

/// Configuration of a logger
#[derive(Clone, Debug, Default, Eq, Fragment, JsonSchema, PartialEq)]
#[fragment(path_overrides(fragment = "crate::config::fragment"))]
#[fragment_attrs(
    derive(
        Clone,
        Debug,
        Default,
        Deserialize,
        JsonSchema,
        PartialEq,
        Merge,
        Serialize
    ),
    merge(path_overrides(merge = "crate::config::merge")),
    serde(rename_all = "camelCase")
)]
pub struct LoggerConfig {
    /// The log level threshold
    ///
    /// Log events with a lower log level are discarded.
    pub level: LogLevel,
}

/// Configuration of a log appender
#[derive(Clone, Debug, Default, Eq, Fragment, JsonSchema, PartialEq)]
#[fragment(path_overrides(fragment = "crate::config::fragment"))]
#[fragment_attrs(
    derive(
        Clone,
        Debug,
        Default,
        Deserialize,
        JsonSchema,
        Merge,
        PartialEq,
        Serialize
    ),
    merge(path_overrides(merge = "crate::config::merge")),
    serde(rename_all = "camelCase")
)]
pub struct AppenderConfig {
    /// The log level threshold
    ///
    /// Log events with a lower log level are discarded.
    pub level: Option<LogLevel>,
}

/// Log levels
#[derive(
    Clone,
    Copy,
    Debug,
    Derivative,
    Deserialize,
    Eq,
    JsonSchema,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize,
)]
#[derivative(Default)]
pub enum LogLevel {
    TRACE,
    DEBUG,
    #[derivative(Default)]
    INFO,
    WARN,
    ERROR,
    FATAL,
    /// Turn logging off
    NONE,
}

impl Atomic for LogLevel {}

impl LogLevel {
    /// Convert the log level to a string understood by Vector
    pub fn to_vector_literal(&self) -> String {
        match self {
            LogLevel::TRACE => "trace",
            LogLevel::DEBUG => "debug",
            LogLevel::INFO => "info",
            LogLevel::WARN => "warn",
            LogLevel::ERROR => "error",
            LogLevel::FATAL => "error",
            LogLevel::NONE => "off",
        }
        .into()
    }

    /// Convert the log level to a string understood by logback
    pub fn to_logback_literal(&self) -> String {
        match self {
            LogLevel::TRACE => "TRACE",
            LogLevel::DEBUG => "DEBUG",
            LogLevel::INFO => "INFO",
            LogLevel::WARN => "WARN",
            LogLevel::ERROR => "ERROR",
            LogLevel::FATAL => "ERROR",
            LogLevel::NONE => "OFF",
        }
        .into()
    }

    /// Convert the log level to a string understood by log4j
    pub fn to_log4j_literal(&self) -> String {
        match self {
            LogLevel::TRACE => "TRACE",
            LogLevel::DEBUG => "DEBUG",
            LogLevel::INFO => "INFO",
            LogLevel::WARN => "WARN",
            LogLevel::ERROR => "ERROR",
            LogLevel::FATAL => "FATAL",
            LogLevel::NONE => "OFF",
        }
        .into()
    }
}

/// Create the default logging configuration
pub fn default_logging<T>() -> LoggingFragment<T>
where
    T: Clone + Display + Ord + strum::IntoEnumIterator,
{
    LoggingFragment {
        enable_vector_agent: Some(true),
        containers: T::iter()
            .map(|container| (container, default_container_log_config()))
            .collect(),
    }
}

/// Create the default logging configuration for a container
pub fn default_container_log_config() -> ContainerLogConfigFragment {
    ContainerLogConfigFragment {
        choice: Some(ContainerLogConfigChoiceFragment::Automatic(
            AutomaticContainerLogConfigFragment {
                loggers: [(
                    AutomaticContainerLogConfig::ROOT_LOGGER.into(),
                    LoggerConfigFragment {
                        level: Some(LogLevel::INFO),
                    },
                )]
                .into(),
                console: Some(AppenderConfigFragment {
                    level: Some(LogLevel::INFO),
                }),
                file: Some(AppenderConfigFragment {
                    level: Some(LogLevel::INFO),
                }),
            },
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::config::{fragment, merge};

    use super::{
        AppenderConfig, AppenderConfigFragment, AutomaticContainerLogConfig,
        AutomaticContainerLogConfigFragment, ConfigMapLogConfig, ConfigMapLogConfigFragment,
        ContainerLogConfig, ContainerLogConfigChoice, ContainerLogConfigChoiceFragment,
        ContainerLogConfigFragment, CustomContainerLogConfig, CustomContainerLogConfigFragment,
        LogLevel,
    };

    #[test]
    fn serialize_container_log_config() {
        // automatic configuration
        assert_eq!(
            "{\"loggers\":{},\"console\":{\"level\":\"INFO\"},\"file\":{\"level\":\"WARN\"}}"
                .to_string(),
            serde_json::to_string(&ContainerLogConfigFragment {
                choice: Some(ContainerLogConfigChoiceFragment::Automatic(
                    AutomaticContainerLogConfigFragment {
                        loggers: BTreeMap::new(),
                        console: Some(AppenderConfigFragment {
                            level: Some(LogLevel::INFO),
                        }),
                        file: Some(AppenderConfigFragment {
                            level: Some(LogLevel::WARN),
                        }),
                    },
                )),
            })
            .unwrap()
        );

        // custom configuration
        assert_eq!(
            "{\"custom\":{\"configMap\":\"configMap\"}}".to_string(),
            serde_json::to_string(&ContainerLogConfigFragment {
                choice: Some(ContainerLogConfigChoiceFragment::Custom(
                    CustomContainerLogConfigFragment {
                        custom: ConfigMapLogConfigFragment {
                            config_map: Some("configMap".into())
                        }
                    },
                )),
            })
            .unwrap()
        );
    }

    #[test]
    fn deserialize_container_log_config() {
        // automatic configuration if only automatic configuration is given
        assert_eq!(
            ContainerLogConfigFragment {
                choice: Some(ContainerLogConfigChoiceFragment::Automatic(
                    AutomaticContainerLogConfigFragment {
                        loggers: BTreeMap::new(),
                        console: Some(AppenderConfigFragment {
                            level: Some(LogLevel::INFO),
                        }),
                        file: Some(AppenderConfigFragment {
                            level: Some(LogLevel::WARN),
                        }),
                    },
                )),
            },
            serde_json::from_str::<ContainerLogConfigFragment>(
                "{\"loggers\":{},\"console\":{\"level\":\"INFO\"},\"file\":{\"level\":\"WARN\"}}"
            )
            .unwrap()
        );

        // custom configuration if only custom configuration is given
        assert_eq!(
            ContainerLogConfigFragment {
                choice: Some(ContainerLogConfigChoiceFragment::Custom(
                    CustomContainerLogConfigFragment {
                        custom: ConfigMapLogConfigFragment {
                            config_map: Some("configMap".into())
                        }
                    }
                )),
            },
            serde_json::from_str::<ContainerLogConfigFragment>(
                "{\"custom\":{\"configMap\":\"configMap\"}}"
            )
            .unwrap()
        );

        // automatic configuration if no configuration is given
        assert_eq!(
            ContainerLogConfigFragment {
                choice: Some(ContainerLogConfigChoiceFragment::Automatic(
                    AutomaticContainerLogConfigFragment {
                        loggers: BTreeMap::new(),
                        console: None,
                        file: None,
                    },
                )),
            },
            serde_json::from_str::<ContainerLogConfigFragment>("{}").unwrap()
        );

        // custom configuration if custom and automatic configurations are given
        assert_eq!(
            ContainerLogConfigFragment {
                choice: Some(ContainerLogConfigChoiceFragment::Custom(
                    CustomContainerLogConfigFragment {
                        custom: ConfigMapLogConfigFragment {
                            config_map: Some("configMap".into())
                        }
                    }
                )),
            },
            serde_json::from_str::<ContainerLogConfigFragment>(
                "{\"custom\":{\"configMap\":\"configMap\"},\"loggers\":{},\"console\":{\"level\":\"INFO\"},\"file\":{\"level\":\"WARN\"}}"
            )
            .unwrap()
        );
    }

    #[test]
    fn merge_automatic_container_log_config_fragment() {
        // no overriding log level + no default log level -> no log level
        assert_eq!(
            AutomaticContainerLogConfigFragment {
                loggers: BTreeMap::new(),
                console: None,
                file: None,
            },
            merge::merge(
                AutomaticContainerLogConfigFragment {
                    loggers: BTreeMap::new(),
                    console: None,
                    file: None,
                },
                &AutomaticContainerLogConfigFragment {
                    loggers: BTreeMap::new(),
                    console: None,
                    file: None,
                }
            )
        );

        // overriding log level + no default log level -> overriding log level
        assert_eq!(
            AutomaticContainerLogConfigFragment {
                loggers: BTreeMap::new(),
                console: Some(AppenderConfigFragment {
                    level: Some(LogLevel::INFO),
                }),
                file: Some(AppenderConfigFragment {
                    level: Some(LogLevel::WARN),
                }),
            },
            merge::merge(
                AutomaticContainerLogConfigFragment {
                    loggers: BTreeMap::new(),
                    console: Some(AppenderConfigFragment {
                        level: Some(LogLevel::INFO),
                    }),
                    file: Some(AppenderConfigFragment {
                        level: Some(LogLevel::WARN),
                    }),
                },
                &AutomaticContainerLogConfigFragment {
                    loggers: BTreeMap::new(),
                    console: None,
                    file: None,
                }
            )
        );

        // no overriding log level + default log level -> default log level
        assert_eq!(
            AutomaticContainerLogConfigFragment {
                loggers: BTreeMap::new(),
                console: Some(AppenderConfigFragment {
                    level: Some(LogLevel::INFO),
                }),
                file: Some(AppenderConfigFragment {
                    level: Some(LogLevel::WARN),
                }),
            },
            merge::merge(
                AutomaticContainerLogConfigFragment {
                    loggers: BTreeMap::new(),
                    console: None,
                    file: None,
                },
                &AutomaticContainerLogConfigFragment {
                    loggers: BTreeMap::new(),
                    console: Some(AppenderConfigFragment {
                        level: Some(LogLevel::INFO),
                    }),
                    file: Some(AppenderConfigFragment {
                        level: Some(LogLevel::WARN),
                    }),
                }
            )
        );

        // overriding log level + default log level -> overriding log level
        assert_eq!(
            AutomaticContainerLogConfigFragment {
                loggers: BTreeMap::new(),
                console: Some(AppenderConfigFragment {
                    level: Some(LogLevel::INFO),
                }),
                file: Some(AppenderConfigFragment {
                    level: Some(LogLevel::ERROR),
                }),
            },
            merge::merge(
                AutomaticContainerLogConfigFragment {
                    loggers: BTreeMap::new(),
                    console: Some(AppenderConfigFragment { level: None }),
                    file: Some(AppenderConfigFragment {
                        level: Some(LogLevel::ERROR),
                    }),
                },
                &AutomaticContainerLogConfigFragment {
                    loggers: BTreeMap::new(),
                    console: Some(AppenderConfigFragment {
                        level: Some(LogLevel::INFO),
                    }),
                    file: Some(AppenderConfigFragment {
                        level: Some(LogLevel::WARN),
                    }),
                }
            )
        );
    }

    #[test]
    fn merge_container_log_config() {
        // overriding automatic config + default custom config -> overriding automatic config
        assert_eq!(
            ContainerLogConfigFragment {
                choice: Some(ContainerLogConfigChoiceFragment::Automatic(
                    AutomaticContainerLogConfigFragment {
                        loggers: BTreeMap::new(),
                        console: Some(AppenderConfigFragment {
                            level: Some(LogLevel::INFO),
                        }),
                        file: Some(AppenderConfigFragment {
                            level: Some(LogLevel::WARN),
                        }),
                    },
                )),
            },
            merge::merge(
                ContainerLogConfigFragment {
                    choice: Some(ContainerLogConfigChoiceFragment::Automatic(
                        AutomaticContainerLogConfigFragment {
                            loggers: BTreeMap::new(),
                            console: Some(AppenderConfigFragment {
                                level: Some(LogLevel::INFO),
                            }),
                            file: Some(AppenderConfigFragment {
                                level: Some(LogLevel::WARN),
                            }),
                        },
                    )),
                },
                &ContainerLogConfigFragment {
                    choice: Some(ContainerLogConfigChoiceFragment::Custom(
                        CustomContainerLogConfigFragment {
                            custom: ConfigMapLogConfigFragment {
                                config_map: Some("configMap".into())
                            }
                        },
                    )),
                }
            )
        );

        // overriding automatic config + default automatic config -> merged automatic config
        assert_eq!(
            ContainerLogConfigFragment {
                choice: Some(ContainerLogConfigChoiceFragment::Automatic(
                    AutomaticContainerLogConfigFragment {
                        loggers: BTreeMap::new(),
                        console: Some(AppenderConfigFragment {
                            level: Some(LogLevel::INFO),
                        }),
                        file: Some(AppenderConfigFragment {
                            level: Some(LogLevel::WARN),
                        }),
                    },
                )),
            },
            merge::merge(
                ContainerLogConfigFragment {
                    choice: Some(ContainerLogConfigChoiceFragment::Automatic(
                        AutomaticContainerLogConfigFragment {
                            loggers: BTreeMap::new(),
                            console: Some(AppenderConfigFragment { level: None }),
                            file: Some(AppenderConfigFragment {
                                level: Some(LogLevel::WARN),
                            }),
                        },
                    )),
                },
                &ContainerLogConfigFragment {
                    choice: Some(ContainerLogConfigChoiceFragment::Automatic(
                        AutomaticContainerLogConfigFragment {
                            loggers: BTreeMap::new(),
                            console: Some(AppenderConfigFragment {
                                level: Some(LogLevel::INFO),
                            }),
                            file: Some(AppenderConfigFragment { level: None }),
                        },
                    )),
                }
            )
        );
    }

    #[test]
    fn validate_automatic_container_log_config() {
        assert_eq!(
            ContainerLogConfig {
                choice: Some(ContainerLogConfigChoice::Automatic(
                    AutomaticContainerLogConfig {
                        loggers: BTreeMap::new(),
                        console: Some(AppenderConfig {
                            level: Some(LogLevel::INFO)
                        }),
                        file: Some(AppenderConfig {
                            level: Some(LogLevel::WARN)
                        }),
                    }
                ))
            },
            fragment::validate::<ContainerLogConfig>(ContainerLogConfigFragment {
                choice: Some(ContainerLogConfigChoiceFragment::Automatic(
                    AutomaticContainerLogConfigFragment {
                        loggers: BTreeMap::new(),
                        console: Some(AppenderConfigFragment {
                            level: Some(LogLevel::INFO),
                        }),
                        file: Some(AppenderConfigFragment {
                            level: Some(LogLevel::WARN),
                        }),
                    },
                )),
            })
            .unwrap()
        );
    }

    #[test]
    fn validate_custom_container_log_config() {
        assert_eq!(
            ContainerLogConfig {
                choice: Some(ContainerLogConfigChoice::Custom(CustomContainerLogConfig {
                    custom: ConfigMapLogConfig {
                        config_map: "configMap".into()
                    }
                }))
            },
            fragment::validate::<ContainerLogConfig>(ContainerLogConfigFragment {
                choice: Some(ContainerLogConfigChoiceFragment::Custom(
                    CustomContainerLogConfigFragment {
                        custom: ConfigMapLogConfigFragment {
                            config_map: Some("configMap".into())
                        }
                    },
                )),
            })
            .unwrap()
        );
    }
}
