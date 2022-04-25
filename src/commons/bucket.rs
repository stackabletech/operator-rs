//! Implementation of the bucket definition as described in
//! https://github.com/stackabletech/documentation/pull/177
//!
//!
use crate::commons::tls::Tls;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct S3Bucket {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionDef>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionDef {
    Inline(S3Connection),
    Reference(String),
}
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct S3Connection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<Tls>,
}

#[cfg(test)]
mod test {
    use crate::commons::bucket::ConnectionDef::Inline;
    use crate::commons::bucket::{S3Bucket, S3Connection};

    #[test]
    fn test_ser_inline() {
        let bucket = S3Bucket {
            bucket_name: Some("test-bucket-name".to_owned()),
            connection: Some(Inline(S3Connection {
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
".to_owned())
    }
}
