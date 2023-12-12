//! We have multiple products (e.g. Trino) that can take a list of usernames + passwords and authenticate users against that list
//! (see <https://trino.io/docs/current/security/password-file.html>). Trino e.g. takes a list of bcrypt passwords.
//!
//! IMPORTANT: Operators should never read Secret contents!!!
//!
//! We do not provide a namespace for the `Secret` reference since we have to mount it into the respective Pods which does not
//! work cross namespace. It might be the case that the current solution has the downside that the `user_credentials_secret` needs to
//! be in the same namespace as the product using it. A solution could be the cluster-scoped SecretClass but that introduces complexity.
//!
//! * We store the credentials as plain text within the Secret. Some products need the credentials as plain text, others hashed.
//!   To achieve a common mechanism we need to store the credentials in plain text.
//! * The secret gets mounted as files. The entrypoint of the product Pod collects them together in the format accepted by the product.
//!   If hashing is needed (e.g. Trino) it hashes as well. If it makes sense, parts are moved to operator-rs.
//! * Restart-controller is enabled and should work as normal.
//! * OPTIONAL: Some product allow hot-reloading the credentials (e.g. Trino). In this case no restart should be done, the secret should update automatically.
//!   Mounted Secrets are updated automatically. When a secret being already consumed in a volume is updated, projected keys are eventually updated as well.
//!   The update time depends on the kubelet syncing period.
//!   This would need additional functionality in restart controller to white- or blacklist certain volumes. Additionally, we would need a sidecar container that
//!   periodically converts the secret contents to the required product format.
//!
//! See <https://github.com/stackabletech/operator-rs/issues/494>
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationProvider {
    /// Secret providing the usernames and passwords.
    /// The Secret must contain an entry for every user, with the key being the username and the value the password in plain text.
    /// It must be located in the same namespace as the product using it.
    pub user_credentials_secret: UserCredentialsSecretRef,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserCredentialsSecretRef {
    /// Name of the Secret.
    pub name: String,
}
