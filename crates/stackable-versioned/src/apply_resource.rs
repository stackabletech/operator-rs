use k8s_openapi::Resource;
use kube::Client;
/// Given a [kube::Client], apply a resource to the server.
///
/// This is especially useful when you have custom requirements for deploying
/// CRDs to clusters which already have a definition.
///
/// For example, you want to prevent stable versions (v1) from having any
/// change.
pub trait ApplyResource: Resource {
    type Error;

    /// Apply a resource to a cluster
    fn apply(&self, kube_client: Client) -> Result<(), Self::Error>;
}
