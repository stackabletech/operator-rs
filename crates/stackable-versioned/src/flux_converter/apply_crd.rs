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
        //    - spec.conversion (this will often change, which is fine (e.g. caBundle rotation))
        //    - spec.group (this should never change)
        //    - spec.names (it is ok to add names, probably not great to remove them, but legit as
        //      we can only keep a limited number because of CR size limitations)
        //    - spec.preserve_unknown_fields (we can be opinionated and reject Some(false)
        //      (and accept None and Some(true)). This is because the field is deprecated in favor
        //      of setting x-preserve-unknown-fields to true in spec.versions\[*\].schema.openAPIV3Schema.
        //      See https://kubernetes.io/docs/tasks/extend-kubernetes/custom-resources/custom-resource-definitions/#field-pruning
        //      for details.
        //    - spec.scope (this should never change)
        //
        // 3. For spec.versions, where "A" is the set of versions currently defined on the stored CRD,
        //    and "B" is the set of versions to be applied...
        //    - A - B: These versions are candidates for removal
        //    - B - A: These versions can be safely appended
        //    - A ∩ B: These versions are likely to change in the following ways:
        //      - New optional fields added (safe for vXalphaY, vXbetaY, and vX)
        //      - Fields changed (can happen in vXalphaY, vXbetaY, but shouldn't in vX)
        //      - Fields removed (can happen in vXalphaY, vXbetaY, but shouldn't in vX)
        //
        // Complete the rest of the owl...
        Ok(())
    }
}
