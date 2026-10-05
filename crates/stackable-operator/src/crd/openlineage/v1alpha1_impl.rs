use snafu::{ResultExt as _, Snafu};

use crate::{
    client::Client,
    crd::openlineage::{
        ResolvedOpenLineageConnection,
        v1alpha1::{
            HttpTransport, InlineConnectionOrReference, OpenLineageConfig, OpenLineageConnection,
        },
    },
};

#[derive(Debug, Snafu)]
pub enum OpenLineageError {
    #[snafu(display("failed to build OpenLineage URL from endpoint '{endpoint}'"))]
    HttpTransportUrl {
        source: url::ParseError,
        endpoint: String,
    },
    #[snafu(display("failed to retrieve OpenLineage connection '{open_lineage_connection}'"))]
    RetrieveOpenLineageConnection {
        #[snafu(source(from(crate::client::Error, Box::new)))]
        source: Box<crate::client::Error>,
        open_lineage_connection: String,
    },
}

impl HttpTransport {
    /// Having it as `const &str` as well, so we don't always allocate a [`String`] just for comparisons
    pub const DEFAULT_PATH: &str = "/api/v1/lineage";

    pub(super) fn default_path() -> String {
        Self::DEFAULT_PATH.to_string()
    }

    /// Build the OpenLineage transport URL (without path) from this transport.
    pub fn url_without_path(&self) -> Result<url::Url, OpenLineageError> {
        let scheme = if self.tls.uses_tls() { "https" } else { "http" };

        let endpoint = format!(
            "{scheme}://{host}:{port}",
            host = self.host,
            port = self.port
        );

        url::Url::parse(&endpoint).context(HttpTransportUrlSnafu { endpoint })
    }

    /// Build the OpenLineage transport URL (with path) from this transport.
    pub fn url(&self) -> Result<url::Url, OpenLineageError> {
        match self.url_without_path() {
            Ok(mut target) => {
                target.set_path(&self.path);
                Ok(target)
            }
            Err(err) => Err(err),
        }
    }
}

impl OpenLineageConfig {
    /// Having it as `const &str` as well, so we don't always allocate a [`String`] just for comparisons
    pub const DEFAULT_NAMESPACE: &str = "default";

    pub(super) fn default_namespace() -> String {
        Self::DEFAULT_NAMESPACE.to_string()
    }
}

impl InlineConnectionOrReference {
    pub async fn resolve(
        self,
        client: &Client,
        namespace: &str,
    ) -> Result<ResolvedOpenLineageConnection, OpenLineageError> {
        match self {
            Self::Inline(inline) => Ok(inline),
            Self::Reference(reference) => {
                let connection_spec = client
                    .get::<OpenLineageConnection>(&reference, namespace)
                    .await
                    .context(RetrieveOpenLineageConnectionSnafu {
                        open_lineage_connection: reference,
                    })?
                    .spec;

                Ok(connection_spec)
            }
        }
    }
}
