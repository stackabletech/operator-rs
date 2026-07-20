use snafu::{ResultExt as _, Snafu};

use crate::{
    client::Client,
    crd::{
        authentication::core::v1alpha1::AuthenticationClass,
        openlineage::{
            ResolvedOpenLineageConnection,
            v1alpha1::{
                InlineConnectionOrReference, OpenLineageConnection, OpenLineageConnectionSpec,
            },
        },
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

    #[snafu(display("failed to retrieve AuthenticationClass '{authentication_class}'"))]
    RetrieveAuthenticationClass {
        #[snafu(source(from(crate::client::Error, Box::new)))]
        source: Box<crate::client::Error>,
        authentication_class: String,
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

    /// Resolves the [`AuthenticationClass`] referenced by this connection, if any.
    ///
    /// Returns `Ok(None)` when no `authenticationClassRef` is configured. The `AuthenticationClass`
    /// is cluster-scoped, so no namespace is required.
    pub async fn resolve_authentication_class(
        &self,
        client: &Client,
    ) -> Result<Option<AuthenticationClass>, OpenLineageError> {
        let Some(authentication_class_ref) = &self.authentication_class_ref else {
            return Ok(None);
        };

        let resolved = AuthenticationClass::resolve(client, authentication_class_ref)
            .await
            .context(RetrieveAuthenticationClassSnafu {
                authentication_class: authentication_class_ref.clone(),
            })?;

        Ok(Some(resolved))
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
