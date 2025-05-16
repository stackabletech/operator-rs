use std::convert::Infallible;

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;

use crate::apply_resource::ApplyResource;

impl ApplyResource for CustomResourceDefinition {
    type Error = Infallible;

    fn apply(&self, _kube_client: kube::Client) -> Result<(), Self::Error> {
        // 1. Using the kube::Client, check if the CRD already exists.
        //    If it does not exist, then simple apply.
        //
        // 2. If the CRD already exists, then get it, and check...
        //    - spec.conversion (this is likely to change, which is fine)
        //    - spec.group (this should probably never change)
        //    - spec.names (it is ok to add names, probably not great to remove them)
        //    - spec.preserve_unknown_fields (is this ok to change?)
        //    - spec.scope (this should probably never change)
        //
        // 3. For spec.versions, where "A" is the sert of versions applied to the server,
        //    and "B" is the set of versions to be applied...
        //    - A - B: These versions are candidates for removal
        //    - B - A: These versions can be safely appended
        //    - A ∩ B: These versions are likely to change in the following ways:
        //      - New fields added (safe for vXalphaY, vXbetaY, and vX)
        //      - Fields changed (can happen in vXalphaY, vXbetaY, but shouldn't in vX)
        //      - Fields removed (can happen in vXalphaY, vXbetaY, but shouldn't in vX)
        //
        // Complete the rest of the owl...
        Ok(())
    }
}
