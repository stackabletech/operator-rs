use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stackable_versioned::versioned;
// ---
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1alpha2"),
    options(k8s(experimental_conversion_tracking))
)]
// ---
pub(crate) mod versioned {
    #[versioned(crd(group = "stackable.tech", doc = "Test"))]
    #[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, CustomResource)]
    pub(crate) struct FooSpec {
        // With conversion tracking enabled, enums only implement TrackingFrom. As such, fields
        // using a versioned enum need to be marked as nested.
        #[versioned(nested)]
        connector: Connector,
    }

    #[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
    pub(crate) enum Connector {
        // The data of this variant is a versioned struct, which tracks changes.
        #[versioned(nested)]
        Iceberg(IcebergConnector),

        // Nested variants with named fields are supported as well.
        #[versioned(nested)]
        Hive { connector: IcebergConnector },

        // Nested tuple variants with multiple fields use the index as an additional path segment.
        #[versioned(nested)]
        Both(IcebergConnector, IcebergConnector),

        // The data of this variant is not versioned and is converted using From.
        Tpch(TpchConnector),

        Tpcds(TpchConnector, TpchConnector),

        Unit,
    }

    #[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
    pub(crate) struct IcebergConnector {
        metastore: Option<String>,

        #[versioned(added(since = "v1alpha2"))]
        rest_catalog_uri: Option<String>,
    }
}
// ---
fn main() {}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct TpchConnector {}
