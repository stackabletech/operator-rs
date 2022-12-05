use std::collections::BTreeMap;
use std::fmt::Display;

use crate::config::fragment::{self, FromFragment};
use crate::config::merge::Atomic;
use crate::config::{fragment::Fragment, merge::Merge};

use derivative::Derivative;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
    pub enable_vector_agent: bool,
    #[fragment_attrs(serde(default))]
    pub containers: BTreeMap<T, ContainerLogConfig>,
}

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
    #[fragment_attrs(serde(flatten))]
    pub choice: Option<ContainerLogConfigChoice>,
}

#[derive(Clone, Debug, Derivative, Eq, JsonSchema, PartialEq)]
#[derivative(Default)]
pub enum ContainerLogConfigChoice {
    Custom(CustomContainerLogConfig),
    #[derivative(Default)]
    Automatic(AutomaticContainerLogConfig),
}

#[derive(Clone, Debug, Derivative, Deserialize, JsonSchema, Merge, PartialEq, Serialize)]
#[derivative(Default)]
#[merge(path_overrides(merge = "crate::config::merge"))]
#[serde(untagged)]
pub enum ContainerLogConfigChoiceFragment {
    Custom(CustomContainerLogConfigFragment),
    #[derivative(Default)]
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
    #[fragment_attrs(serde(default))]
    pub config_map: String,
}

#[derive(Clone, Debug, Default, Eq, Fragment, JsonSchema, PartialEq)]
#[fragment(path_overrides(fragment = "crate::config::fragment"))]
#[fragment_attrs(
    derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize),
    serde(rename_all = "camelCase")
)]
pub struct AutomaticContainerLogConfig {
    #[fragment_attrs(serde(default))]
    pub loggers: BTreeMap<String, LoggerConfig>,
    pub console: Option<AppenderConfig>,
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
    pub const ROOT_LOGGER: &'static str = "ROOT";

    pub fn root_log_level(&self) -> LogLevel {
        self.loggers
            .get(Self::ROOT_LOGGER)
            .map(|root| root.level.to_owned())
            .unwrap_or_default()
    }
}

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
    pub level: LogLevel,
}

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
    pub level_threshold: Option<LogLevel>,
}

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
    NONE,
}

impl Atomic for LogLevel {}

impl LogLevel {
    pub fn to_vector_literal(&self) -> String {
        match self {
            LogLevel::TRACE => "TRACE",
            LogLevel::DEBUG => "DEBUG",
            LogLevel::INFO => "INFO",
            LogLevel::WARN => "WARN",
            LogLevel::ERROR => "ERROR",
            LogLevel::FATAL => "ERROR",
            LogLevel::NONE => "ERROR",
        }
        .into()
    }

    pub fn to_logback_literal(&self) -> String {
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
                    level_threshold: Some(LogLevel::INFO),
                }),
                file: Some(AppenderConfigFragment {
                    level_threshold: Some(LogLevel::INFO),
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
        assert_eq!(
            "{\"loggers\":{},\"console\":{\"levelThreshold\":\"INFO\"},\"file\":{\"levelThreshold\":\"WARN\"}}".to_string(),
            serde_json::to_string(&ContainerLogConfigFragment {
                choice: Some(ContainerLogConfigChoiceFragment::Automatic(
                    AutomaticContainerLogConfigFragment {
                        loggers: BTreeMap::new(),
                        console: Some(AppenderConfigFragment {
                            level_threshold: Some(LogLevel::INFO),
                        }),
                        file: Some(AppenderConfigFragment {
                            level_threshold: Some(LogLevel::WARN),
                        }),
                    },
                )),
            })
            .unwrap()
        );

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
        assert_eq!(
            ContainerLogConfigFragment {
                choice: Some(ContainerLogConfigChoiceFragment::Automatic(
                    AutomaticContainerLogConfigFragment {
                        loggers: BTreeMap::new(),
                        console: Some(AppenderConfigFragment {
                            level_threshold: Some(LogLevel::INFO),
                        }),
                        file: Some(AppenderConfigFragment {
                            level_threshold: Some(LogLevel::WARN),
                        }),
                    },
                )),
            },
            serde_json::from_str::<ContainerLogConfigFragment>(
                "{\"loggers\":{},\"console\":{\"levelThreshold\":\"INFO\"},\"file\":{\"levelThreshold\":\"WARN\"}}"
            )
            .unwrap()
        );

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
                "{\"custom\":{\"configMap\":\"configMap\"},\"loggers\":{},\"console\":{\"levelThreshold\":\"INFO\"},\"file\":{\"levelThreshold\":\"WARN\"}}"
            )
            .unwrap()
        );
    }

    #[test]
    fn merge_automatic_container_log_config_fragment() {
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
        assert_eq!(
            AutomaticContainerLogConfigFragment {
                loggers: BTreeMap::new(),
                console: Some(AppenderConfigFragment {
                    level_threshold: Some(LogLevel::INFO),
                }),
                file: Some(AppenderConfigFragment {
                    level_threshold: Some(LogLevel::WARN),
                }),
            },
            merge::merge(
                AutomaticContainerLogConfigFragment {
                    loggers: BTreeMap::new(),
                    console: Some(AppenderConfigFragment {
                        level_threshold: Some(LogLevel::INFO),
                    }),
                    file: Some(AppenderConfigFragment {
                        level_threshold: Some(LogLevel::WARN),
                    }),
                },
                &AutomaticContainerLogConfigFragment {
                    loggers: BTreeMap::new(),
                    console: None,
                    file: None,
                }
            )
        );
        assert_eq!(
            AutomaticContainerLogConfigFragment {
                loggers: BTreeMap::new(),
                console: Some(AppenderConfigFragment {
                    level_threshold: Some(LogLevel::INFO),
                }),
                file: Some(AppenderConfigFragment {
                    level_threshold: Some(LogLevel::WARN),
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
                        level_threshold: Some(LogLevel::INFO),
                    }),
                    file: Some(AppenderConfigFragment {
                        level_threshold: Some(LogLevel::WARN),
                    }),
                }
            )
        );
        assert_eq!(
            AutomaticContainerLogConfigFragment {
                loggers: BTreeMap::new(),
                console: Some(AppenderConfigFragment {
                    level_threshold: Some(LogLevel::INFO),
                }),
                file: Some(AppenderConfigFragment {
                    level_threshold: Some(LogLevel::ERROR),
                }),
            },
            merge::merge(
                AutomaticContainerLogConfigFragment {
                    loggers: BTreeMap::new(),
                    console: Some(AppenderConfigFragment {
                        level_threshold: None,
                    }),
                    file: Some(AppenderConfigFragment {
                        level_threshold: Some(LogLevel::ERROR),
                    }),
                },
                &AutomaticContainerLogConfigFragment {
                    loggers: BTreeMap::new(),
                    console: Some(AppenderConfigFragment {
                        level_threshold: Some(LogLevel::INFO),
                    }),
                    file: Some(AppenderConfigFragment {
                        level_threshold: Some(LogLevel::WARN),
                    }),
                }
            )
        );
    }

    #[test]
    fn merge_container_log_config() {
        assert_eq!(
            ContainerLogConfigFragment {
                choice: Some(ContainerLogConfigChoiceFragment::Automatic(
                    AutomaticContainerLogConfigFragment {
                        loggers: BTreeMap::new(),
                        console: Some(AppenderConfigFragment {
                            level_threshold: Some(LogLevel::INFO),
                        }),
                        file: Some(AppenderConfigFragment {
                            level_threshold: Some(LogLevel::WARN),
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
                                level_threshold: Some(LogLevel::INFO),
                            }),
                            file: Some(AppenderConfigFragment {
                                level_threshold: Some(LogLevel::WARN),
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

        assert_eq!(
            ContainerLogConfigFragment {
                choice: Some(ContainerLogConfigChoiceFragment::Automatic(
                    AutomaticContainerLogConfigFragment {
                        loggers: BTreeMap::new(),
                        console: Some(AppenderConfigFragment {
                            level_threshold: Some(LogLevel::INFO),
                        }),
                        file: Some(AppenderConfigFragment {
                            level_threshold: Some(LogLevel::WARN),
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
                                level_threshold: None,
                            }),
                            file: Some(AppenderConfigFragment {
                                level_threshold: Some(LogLevel::WARN),
                            }),
                        },
                    )),
                },
                &ContainerLogConfigFragment {
                    choice: Some(ContainerLogConfigChoiceFragment::Automatic(
                        AutomaticContainerLogConfigFragment {
                            loggers: BTreeMap::new(),
                            console: Some(AppenderConfigFragment {
                                level_threshold: Some(LogLevel::INFO),
                            }),
                            file: Some(AppenderConfigFragment {
                                level_threshold: None,
                            }),
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
                            level_threshold: Some(LogLevel::INFO)
                        }),
                        file: Some(AppenderConfig {
                            level_threshold: Some(LogLevel::WARN)
                        }),
                    }
                ))
            },
            fragment::validate::<ContainerLogConfig>(ContainerLogConfigFragment {
                choice: Some(ContainerLogConfigChoiceFragment::Automatic(
                    AutomaticContainerLogConfigFragment {
                        loggers: BTreeMap::new(),
                        console: Some(AppenderConfigFragment {
                            level_threshold: Some(LogLevel::INFO),
                        }),
                        file: Some(AppenderConfigFragment {
                            level_threshold: Some(LogLevel::WARN),
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
