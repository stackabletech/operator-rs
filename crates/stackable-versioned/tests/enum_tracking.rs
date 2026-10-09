use kube::{
    CustomResource,
    core::{conversion::ConversionReview, response::StatusSummary},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stackable_versioned::versioned;

#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1alpha2"),
    options(k8s(experimental_conversion_tracking))
)]
pub mod versioned {
    #[versioned(crd(group = "test.stackable.tech", doc = "Test"))]
    #[derive(Clone, Debug, CustomResource, Deserialize, JsonSchema, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct CatalogSpec {
        #[versioned(nested)]
        connector: Connector,
    }

    #[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub enum Connector {
        #[versioned(nested)]
        Iceberg(IcebergConnector),

        Tpch(TpchConnector),
    }

    #[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct IcebergConnector {
        #[versioned(changed(
            since = "v1alpha2",
            from_name = "metastore",
            from_type = "Option<String>"
        ))]
        catalog: IcebergCatalog,
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct TpchConnector {}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IcebergCatalog {
    Rest { uri: String },
    HiveMetastore { config_map: String },
    UserProvided {},
}

// Before v1alpha2, only Hive metastores were supported. Other catalogs had to be configured by
// users manually, so there is no way to represent a REST catalog in v1alpha1.
impl From<IcebergCatalog> for Option<String> {
    fn from(catalog: IcebergCatalog) -> Self {
        match catalog {
            IcebergCatalog::HiveMetastore { config_map } => Some(config_map),
            IcebergCatalog::Rest { .. } | IcebergCatalog::UserProvided {} => None,
        }
    }
}

impl From<Option<String>> for IcebergCatalog {
    fn from(metastore: Option<String>) -> Self {
        match metastore {
            Some(config_map) => Self::HiveMetastore { config_map },
            None => Self::UserProvided {},
        }
    }
}

impl stackable_versioned::test_utils::RoundtripTestData for v1alpha1::CatalogSpec {
    fn roundtrip_test_data() -> Vec<Self> {
        vec![
            Self {
                connector: v1alpha1::Connector::Iceberg(v1alpha1::IcebergConnector {
                    metastore: Some("hive".to_owned()),
                }),
            },
            Self {
                connector: v1alpha1::Connector::Iceberg(v1alpha1::IcebergConnector {
                    metastore: None,
                }),
            },
            Self {
                connector: v1alpha1::Connector::Tpch(TpchConnector {}),
            },
        ]
    }
}

impl stackable_versioned::test_utils::RoundtripTestData for v1alpha2::CatalogSpec {
    fn roundtrip_test_data() -> Vec<Self> {
        vec![
            // The REST catalog can not be represented in v1alpha1. It is tracked in the status
            // and restored when upgrading again.
            Self {
                connector: v1alpha2::Connector::Iceberg(v1alpha2::IcebergConnector {
                    catalog: IcebergCatalog::Rest {
                        uri: "http://rest-catalog:8181".to_owned(),
                    },
                }),
            },
            Self {
                connector: v1alpha2::Connector::Iceberg(v1alpha2::IcebergConnector {
                    catalog: IcebergCatalog::HiveMetastore {
                        config_map: "hive".to_owned(),
                    },
                }),
            },
            Self {
                connector: v1alpha2::Connector::Iceberg(v1alpha2::IcebergConnector {
                    catalog: IcebergCatalog::UserProvided {},
                }),
            },
            Self {
                connector: v1alpha2::Connector::Tpch(TpchConnector {}),
            },
        ]
    }
}

#[test]
fn tracks_values_through_enum_variants() {
    let review: ConversionReview = serde_json::from_value(serde_json::json!({
        "kind": "ConversionReview",
        "apiVersion": "apiextensions.k8s.io/v1",
        "request": {
            "uid": "c4e55572-ee1f-4e94-9097-28936985d45f",
            "desiredAPIVersion": "test.stackable.tech/v1alpha1",
            "objects": [{
                "apiVersion": "test.stackable.tech/v1alpha2",
                "kind": "Catalog",
                "metadata": {},
                "spec": {
                    "connector": {
                        "iceberg": {
                            "catalog": {
                                "rest": {
                                    "uri": "http://rest-catalog:8181"
                                }
                            }
                        }
                    }
                }
            }]
        }
    }))
    .expect("conversion review must be valid");

    let response = Catalog::try_convert(review)
        .response
        .expect("v1alpha1 review must have a response");

    assert_eq!(response.result.status, Some(StatusSummary::Success));

    let object = response
        .converted_objects
        .first()
        .expect("there must be at least one object");

    assert_eq!(
        object["spec"]["connector"]["iceberg"]["metastore"],
        serde_json::Value::Null
    );
    assert_eq!(
        object["status"]["changedValues"]["upgrades"]["v1alpha2"],
        serde_json::json!([{
            "jsonPath": "$.connector.Iceberg.catalog",
            "value": {
                "rest": {
                    "uri": "http://rest-catalog:8181"
                }
            },
            "downgradedValue": null
        }])
    );
}
