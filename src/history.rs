// Modeled after K8s: pkg/controller/history/controller_history.go

use crate::client::Client;
use crate::controller_ref::get_controller_of;
use crate::error::OperatorResult;
use crate::object_to_owner_reference;
use k8s_openapi::api::apps::v1::ControllerRevision;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use k8s_openapi::apimachinery::pkg::runtime::RawExtension;
use kube::api::{Meta, ObjectMeta};
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
    let revisions = client.list(Meta::namespace(resource)).await?;
    let owner_uid = resource.meta().uid.as_ref().unwrap(); // TODO: Error handling
    let mut owned = vec![];
    for revision in revisions {
        if !matches!(get_controller_of(&revision), Some(OwnerReference { uid, ..}) if uid == owner_uid)
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

pub async fn create_controller_revision<T>(
    client: &Client,
    parent: &T,
    data: RawExtension,
    revision: i64,
) -> OperatorResult<ControllerRevision>
where
    T: Meta + Hash,
{
    let mut hasher = DefaultHasher::new();
    parent.hash(&mut hasher);
    let cr = ControllerRevision {
        data: Some(data),
        metadata: ObjectMeta {
            name: Some(controller_revision_name(
                &Meta::name(parent),
                &format!("{:x}", hasher.finish()),
            )),
            namespace: Meta::namespace(parent),
            owner_references: Some(vec![object_to_owner_reference::<T>(parent.meta().clone())?]),
            ..ObjectMeta::default()
        },
        revision,
    };

    client.create(&cr).await // TODO: Retry logic on conflict?
}

/// Returns a formatted name for a ControllerRevision
pub fn controller_revision_name(prefix: &str, hash: &str) -> String {
    // A name can be a maximum of 253 characters
    // Kubernetes (function `ControllerRevisionName`) trims the prefix to 223 characters
    // to have enough room for the hash, ours is shorter (16 chars) but I chose to keep
    // the same restriction and trim at 223
    format!("{:.223}-{}", prefix, hash)
}
