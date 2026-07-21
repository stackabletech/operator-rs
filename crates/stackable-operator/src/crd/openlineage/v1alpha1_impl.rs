use snafu::{ResultExt as _, Snafu};

use crate::{
    client::Client,
    crd::openlineage::{
        ResolvedOpenLineageConnection,
        v1alpha1::{InlineConnectionOrReference, OpenLineageConnection, OpenLineageConnectionSpec},
    },
};

#[derive(Debug, Snafu)]
pub enum OpenLineageError {
    #[snafu(display("failed to retrieve OpenLineage connection '{open_lineage_connection}'"))]
    RetrieveOpenLineageConnection {
        #[snafu(source(from(crate::client::Error, Box::new)))]
        source: Box<crate::client::Error>,
        open_lineage_connection: String,
    },
}

impl OpenLineageConnectionSpec {
    /// Build the OpenLineage transport URL from this connection.
    ///
    /// The scheme is `https` when TLS server verification is configured
    /// (`tls.verification.server`), otherwise `http`.
    pub fn transport_url(&self) -> String {
        let scheme = if self.tls.uses_tls_verification() {
            "https"
        } else {
            "http"
        };

        format!(
            "{scheme}://{host}:{port}",
            host = self.host,
            port = self.port
        )
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
