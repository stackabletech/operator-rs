//! Implementation of the bucket definition as described in
//! https://github.com/stackabletech/documentation/pull/177
//!
//!
use crate::commons::tls::Tls;
use crate::error;
use crate::{client::Client, error::OperatorResult};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, CustomResource, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[kube(
    group = "s3.stackable.tech",
    version = "v1alpha1",
    kind = "S3Bucket",
    plural = "s3buckets",
    crates(
        kube_core = "kube::core",
        k8s_openapi = "k8s_openapi",
        schemars = "schemars"
    ),
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct S3BucketSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionDef>,
}

impl S3BucketSpec {
    pub async fn get(
        resource_name: &str,
        client: &Client,
        namespace: Option<&str>,
    ) -> OperatorResult<S3Bucket> {
        client
            .get::<S3Bucket>(resource_name, namespace)
            .await
            .map_err(|_source| error::Error::MissingS3Bucket {
                name: resource_name.to_string(),
            })
    }

    pub async fn secret_class(&self, client: &Client, namespace: Option<String>) -> Option<String> {
        match self.connection.as_ref() {
            Some(ConnectionDef::Inline(S3ConnectionSpec { secret_class, .. })) => {
                secret_class.clone()
            }
            Some(ConnectionDef::Reference(s3_conn_ref)) => {
                S3Connection::get(s3_conn_ref.as_ref(), client, namespace)
                    .await
                    .map_or(None, |s3c| s3c.spec.secret_class)
            }
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum S3BucketDef {
    Inline(S3BucketSpec),
    Reference(String),
}

impl S3BucketDef {
    pub async fn secret_class(&self, client: &Client, namespace: Option<String>) -> Option<String> {
        match self {
            S3BucketDef::Inline(s3_bucket) => s3_bucket.secret_class(client, namespace).await,
            S3BucketDef::Reference(_s3_bucket) => todo!("get secret_class from bucket ref"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionDef {
    Inline(S3ConnectionSpec),
    Reference(String),
}

#[derive(CustomResource, Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[kube(
    group = "s3.stackable.tech",
    version = "v1alpha1",
    kind = "S3Connection",
    plural = "s3connections",
    crates(
        kube_core = "kube::core",
        k8s_openapi = "k8s_openapi",
        schemars = "schemars"
    ),
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct S3ConnectionSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<Tls>,
}
impl S3Connection {
    pub async fn get(
        resource_name: &str,
        client: &Client,
        namespace: Option<String>,
    ) -> OperatorResult<Self> {
        client
            .get::<Self>(resource_name, namespace.as_deref())
            .await
            .map_err(|_source| error::Error::MissingS3Connection {
                name: resource_name.to_string(),
            })
    }
}

#[cfg(test)]
mod test {
    use crate::commons::s3::ConnectionDef::Inline;
    use crate::commons::s3::{S3BucketSpec, S3ConnectionSpec};

    #[test]
    fn test_ser_inline() {
        let bucket = S3BucketSpec {
            bucket_name: Some("test-bucket-name".to_owned()),
            connection: Some(Inline(S3ConnectionSpec {
                host: Some("host".to_owned()),
                port: Some(8080),
                secret_class: None,
                tls: None,
            })),
        };

        assert_eq!(
            serde_yaml::to_string(&bucket).unwrap(),
            "---
bucketName: test-bucket-name
connection:
  inline:
    host: host
    port: 8080
"
            .to_owned()
        )
    }
}
