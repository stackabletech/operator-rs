use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stackable_versioned::versioned;

mod v1alpha1_impl;

#[versioned(version(name = "v1alpha1"))]
pub mod versioned {
    // This makes v1alpha1 versions of all authentication providers available to the
    // AuthenticationClassProvider enum below.
    mod v1alpha1 {
        use crate::crd::authentication::{kerberos, ldap, oidc, r#static, tls};
    }
    /// The Stackable Platform uses the AuthenticationClass as a central mechanism to handle user
    /// authentication across supported products.
    ///
    /// The authentication mechanism needs to be configured only in the AuthenticationClass which is
    /// then referenced in the product. Multiple different authentication providers are supported.
    /// Learn more in the [authentication concept documentation][1] and the
    /// [Authentication with OpenLDAP tutorial][2].
    ///
    /// [1]: DOCS_BASE_URL_PLACEHOLDER/concepts/authentication
    /// [2]: DOCS_BASE_URL_PLACEHOLDER/tutorials/authentication_with_openldap
    #[versioned(k8s(
        group = "authentication.stackable.tech",
        plural = "authenticationclasses",
        crates(
            kube_core = "kube::core",
            k8s_openapi = "k8s_openapi",
            schemars = "schemars"
        )
    ))]
    #[derive(
        Clone,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
        CustomResource,
        Deserialize,
        JsonSchema,
        Serialize,
    )]
    #[serde(rename_all = "camelCase")]
    pub struct AuthenticationClassSpec {
        /// Provider used for authentication like LDAP or Kerberos.
        pub provider: AuthenticationClassProvider,
    }

    #[derive(
        Clone,
        Debug,
        Deserialize,
        strum::Display,
        Eq,
        Hash,
        JsonSchema,
        Ord,
        PartialEq,
        PartialOrd,
        Serialize,
    )]
    #[serde(rename_all = "camelCase")]
    #[allow(clippy::large_enum_variant)]
    pub enum AuthenticationClassProvider {
        /// The [static provider](https://DOCS_BASE_URL_PLACEHOLDER/concepts/authentication#_static)
        /// is used to configure a static set of users, identified by username and password.
        Static(r#static::v1alpha1::AuthenticationProvider),

        /// The [LDAP provider](DOCS_BASE_URL_PLACEHOLDER/concepts/authentication#_ldap).
        /// There is also the ["Authentication with LDAP" tutorial](DOCS_BASE_URL_PLACEHOLDER/tutorials/authentication_with_openldap)
        /// where you can learn to configure Superset and Trino with OpenLDAP.
        Ldap(ldap::v1alpha1::AuthenticationProvider),

        /// The OIDC provider can be used to configure OpenID Connect.
        Oidc(oidc::v1alpha1::AuthenticationProvider),

        /// The [TLS provider](DOCS_BASE_URL_PLACEHOLDER/concepts/authentication#_tls).
        /// The TLS AuthenticationClass is used when users should authenticate themselves with a TLS certificate.
        Tls(tls::v1alpha1::AuthenticationProvider),

        /// The [Kerberos provider](DOCS_BASE_URL_PLACEHOLDER/concepts/authentication#_kerberos).
        /// The Kerberos AuthenticationClass is used when users should authenticate themselves via Kerberos.
        Kerberos(kerberos::v1alpha1::AuthenticationProvider),
    }

    /// Common [`v1alpha1::ClientAuthenticationDetails`] which is specified at the client/ product
    /// cluster level. It provides a name (key) to resolve a particular [`AuthenticationClass`].
    /// Additionally, it provides authentication provider specific configuration (OIDC and LDAP for
    /// example).
    ///
    /// If the product needs additional (product specific) authentication options, it is recommended
    /// to wrap this struct and use `#[serde(flatten)]` on the field.
    ///
    /// Additionally, it might be the case that special fields are needed in the contained structs,
    /// such as [`oidc::v1alpha1::ClientAuthenticationOptions`]. To be able to add custom fields in
    /// that structs without serde(flattening) multiple structs, they are generic, so you can add
    /// additional attributes if needed.
    ///
    /// ### Example
    ///
    /// ```
    /// # use schemars::JsonSchema;
    /// # use serde::{Deserialize, Serialize};
    /// use stackable_operator::crd::authentication::core::v1alpha1;
    ///
    /// #[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    /// #[serde(rename_all = "camelCase")]
    /// pub struct SupersetAuthenticationClass {
    ///     pub user_registration_role: String,
    ///     pub user_registration: bool,
    ///
    ///     #[serde(flatten)]
    ///     pub common: v1alpha1::ClientAuthenticationDetails,
    /// }
    /// ```
    #[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    #[schemars(description = "")]
    pub struct ClientAuthenticationDetails<O = ()> {
        /// Name of the [AuthenticationClass](https://docs.stackable.tech/home/nightly/concepts/authentication) used to
        /// authenticate users.
        //
        // To get the concrete [`AuthenticationClass`], we must resolve it. This resolution can be achieved by using
        // [`ClientAuthenticationDetails::resolve_class`].
        #[serde(rename = "authenticationClass")]
        authentication_class_ref: String,

        /// This field contains OIDC-specific configuration. It is only required in case OIDC is used.
        //
        // Use [`ClientAuthenticationDetails::oidc_or_error`] to get the value or report an error to the user.
        // TODO: Ideally we want this to be an enum once other `ClientAuthenticationOptions` are added, so
        // that user can not configure multiple options at the same time (yes we are aware that this makes a
        // changing the type of an AuthenticationClass harder).
        // This is a non-breaking change though :)
        oidc: Option<oidc::v1alpha1::ClientAuthenticationOptions<O>>,
    }
}
