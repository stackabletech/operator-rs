use snafu::ResultExt as _;

use crate::{
    client::Client,
    crd::s3::{
        connection::{ConnectionError, ResolvedConnection, RetrieveS3ConnectionSnafu},
        v1alpha1::{InlineConnectionOrReference, Region, S3Connection},
    },
};

impl Region {
    /// Having it as `const &str` as well, so we don't always allocate a [`String`] just for comparisons
    pub const DEFAULT_REGION_NAME: &str = "us-east-1";

    pub(super) fn default_region_name() -> String {
        Self::DEFAULT_REGION_NAME.to_string()
    }

    /// Returns if the region sticks to the Stackable defaults.
    ///
    /// Some products don't really support configuring the region.
    /// This function can be used to determine if a warning or error should be raised to inform the
    /// user of this situation.
    pub fn is_default_config(&self) -> bool {
        self.name == Self::DEFAULT_REGION_NAME
    }
}

impl Default for Region {
    fn default() -> Self {
        Self {
            name: Self::default_region_name(),
        }
    }
}

impl InlineConnectionOrReference {
    pub async fn resolve(
        self,
        client: &Client,
        namespace: &str,
    ) -> Result<ResolvedConnection, ConnectionError> {
        match self {
            Self::Inline(inline) => Ok(inline),
            Self::Reference(reference) => {
                let connection_spec = client
                    .get::<S3Connection>(&reference, namespace)
                    .await
                    .context(RetrieveS3ConnectionSnafu {
                        s3_connection: reference,
                    })?
                    .spec;

                Ok(connection_spec)
            }
        }
    }
}
