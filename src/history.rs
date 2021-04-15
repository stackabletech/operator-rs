// Modeled after K8s: pkg/controller/history/controller_history.go

use crate::client::Client;
use crate::controller_ref;
use crate::error::OperatorResult;
use crate::{k8s_errors, metadata};

use k8s_openapi::api::apps::v1::ControllerRevision;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use k8s_openapi::apimachinery::pkg::runtime::RawExtension;
use kube::api::{ListParams, Meta, ObjectMeta};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Returns a list of all `ControllerRevision` resources that have an `OwnerReference` which points to the passed in resource as its controller.
pub async fn list_controller_revisions<T>(
    client: &Client,
    //    selector: LabelSelector, // TODO: Need to support labels because otherwise we list all ControllerRevisions!
    resource: &T,
) -> OperatorResult<Vec<ControllerRevision>>
where
    T: Meta,
{
    let revisions = client
        .list(Meta::namespace(resource).as_deref(), &ListParams::default())
        .await?;
    let owner_uid = resource.meta().uid.as_ref().unwrap(); // TODO: Error handling
    let mut owned = vec![];
    for revision in revisions {
        if !matches!(controller_ref::get_controller_of(&revision), Some(OwnerReference { uid, ..}) if uid == owner_uid)
        {
            owned.push(revision);
        }
    }

    Ok(owned)
}

/// Sorts the provided vector (in-place) by revision, creation timestamp and name (in that priority order)
pub fn sort_controller_revisions(revisions: &mut Vec<ControllerRevision>) {
    revisions.sort_by(|a, b| {
        a.revision.cmp(&b.revision).then(
            a.metadata
                .creation_timestamp
                .cmp(&b.metadata.creation_timestamp)
                .then(a.metadata.name.cmp(&b.metadata.name)),
        )
    });
    revisions.reverse();
}

/// Finds the next valid revision number based on the passed in revisions.
/// If there are no revisions the next one will be 1" otherwise it is 1 greater than the last one.
/// This assumes that the list has been sorted by `revision`.
pub fn next_revision(revisions: &[ControllerRevision]) -> i64 {
    match revisions.first() {
        None => 1,
        Some(revision) => revision.revision + 1,
    }
}

/// Creates a new `ControllerRevision` for the passed in object/data.
///
/// * `parent` is the object that owns the new `ControllerRevision`, it'll also be in the same namespace and have its name derived from it
/// * `data` is the actual serialized data to put into the `ControllerRevision` object, this should always be related to the ~parent` object
/// * `revision` is the revision number to use for this new object, if there are collisions it'll automatically use a collision counter to change the hashS
// TODO:: K8s stores the `collision_count` in the `Status` field of the object but I don't ee an urgent need at the moment
pub async fn create_controller_revision<T>(
    client: &Client,
    parent: &T,
    data: RawExtension,
    revision: i64,
) -> OperatorResult<ControllerRevision>
where
    T: Meta + Hash,
{
    let mut cr = ControllerRevision {
        data: Some(data),
        metadata: ObjectMeta {
            name: None,
            namespace: Meta::namespace(parent),
            owner_references: Some(vec![metadata::object_to_owner_reference::<T>(
                parent.meta(),
                true,
            )?]),
            ..ObjectMeta::default()
        },
        revision,
    };

    let mut collision_count = 0;
    loop {
        let mut hasher = DefaultHasher::new();
        parent.hash(&mut hasher);
        collision_count.hash(&mut hasher);
        let hash = hasher.finish();

        let name = controller_revision_name(&Meta::name(parent), &format!("{:x}", hash));
        cr.metadata.name = Some(name);

        let result = client.create(&cr).await;
        if k8s_errors::is_already_exists(&result) {
            collision_count += 1;
            continue;
        } else {
            return result;
        }
    }
}

/// Returns a formatted name for a ControllerRevision
pub fn controller_revision_name(prefix: &str, hash: &str) -> String {
    // A name can be a maximum of 253 characters
    // Kubernetes (function `ControllerRevisionName`) trims the prefix to 223 characters
    // to have enough room for the hash, ours is shorter (16 chars) but I chose to keep
    // the same restriction and trim at 223
    format!("{:.223}-{}", prefix, hash)
}
