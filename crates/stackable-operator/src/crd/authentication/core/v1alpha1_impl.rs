use snafu::{OptionExt as _, Snafu};

use crate::{
    client::Client,
    crd::authentication::{
        core::v1alpha1::{AuthenticationClass, ClientAuthenticationDetails},
        oidc::v1alpha1 as oidc_v1alpha1,
    },
};

type Result<T, E = Error> = std::result::Result<T, E>;

// NOTE (@Techassi): Where is the best place to put this?
#[derive(Debug, PartialEq, Snafu)]
pub enum Error {
    #[snafu(display("authentication details for OIDC were not specified. The AuthenticationClass {auth_class_name:?} uses an OIDC provider, you need to specify OIDC authentication details (such as client credentials) as well"))]
    OidcAuthenticationDetailsNotSpecified { auth_class_name: String },
}

impl AuthenticationClass {
    pub async fn resolve(
        client: &Client,
        authentication_class_name: &str,
    ) -> crate::client::Result<AuthenticationClass> {
        client
            .get::<AuthenticationClass>(authentication_class_name, &()) // AuthenticationClass has ClusterScope
            .await
    }
}

impl<O> ClientAuthenticationDetails<O> {
    /// Resolves this specific [`AuthenticationClass`]. Usually products support
    /// a list of authentication classes, which individually need to be resolved.crate::client
    pub async fn resolve_class(
        &self,
        client: &Client,
    ) -> crate::client::Result<AuthenticationClass> {
        AuthenticationClass::resolve(client, &self.authentication_class_ref).await
    }

    pub fn authentication_class_name(&self) -> &String {
        &self.authentication_class_ref
    }

    /// In case OIDC is configured, the user *needs* to provide some connection details,
    /// such as the client credentials. Call this function in case the user has configured
    /// OIDC, as it will error out then the OIDC client details are missing.
    pub fn oidc_or_error(
        &self,
        auth_class_name: &str,
    ) -> Result<&oidc_v1alpha1::ClientAuthenticationOptions<O>> {
        self.oidc
            .as_ref()
            .with_context(|| OidcAuthenticationDetailsNotSpecifiedSnafu {
                auth_class_name: auth_class_name.to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use crate::crd::authentication::{
        core::v1alpha1::AuthenticationClassProvider, kerberos::v1alpha1 as kerberos_v1alpha1,
        tls::v1alpha1 as tls_v1alpha1,
    };

    #[test]
    fn provider_to_string() {
        let tls_provider = AuthenticationClassProvider::Tls(tls_v1alpha1::AuthenticationProvider {
            client_cert_secret_class: None,
        });
        assert_eq!("Tls", tls_provider.to_string());

        let kerberos_provider =
            AuthenticationClassProvider::Kerberos(kerberos_v1alpha1::AuthenticationProvider {
                kerberos_secret_class: "kerberos".to_string(),
            });
        assert_eq!("Kerberos", kerberos_provider.to_string());
    }
}
