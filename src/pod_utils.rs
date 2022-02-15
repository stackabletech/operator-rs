/*
We consider the functions in this module to be somewhat useful.
Not necessarily these exact functions, but things in here will probably stick around in some
shape or form.
 */

use kube::api::Resource;
use kube::runtime::reflector::ObjectRef;

/// Returns a name that is suitable for directly passing to a log macro.
///
/// It'll contain the kind, API group, namespace, and the object.
/// Example output: `Deployment.v1.apps/my-deployment.my-namespace`
pub fn get_log_name<T>(obj: &T) -> String
where
    T: Resource,
    T::DynamicType: Default,
{
    ObjectRef::from_obj(obj).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{ObjectMetaBuilder, PodBuilder};

    #[test]
    fn test_get_log_name() {
        let mut pod = PodBuilder::new()
            .metadata(ObjectMetaBuilder::new().name("bar").build())
            .build()
            .unwrap();
        assert_eq!("Pod.v1./bar", get_log_name(&pod));

        pod.metadata.namespace = Some("foo".to_string());
        assert_eq!("Pod.v1./bar.foo", get_log_name(&pod));
    }
}
