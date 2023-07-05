use crate::{
    builder::kvp::LabelListBuilder,
    constants::labels::{
        LABEL_KEY_NAME_APP_COMPONENT, LABEL_KEY_NAME_APP_INSTANCE, LABEL_KEY_NAME_APP_MANAGED_BY,
        LABEL_KEY_NAME_APP_NAME, LABEL_KEY_NAME_APP_ROLE_GROUP, LABEL_KEY_NAME_APP_VERSION,
        LABEL_KEY_PREFIX_APP_KUBERNETES,
    },
    types::{Label, LabelParseError},
    utils::format_full_controller_name,
};

use kube::api::{Resource, ResourceExt};
use std::collections::BTreeMap;

#[cfg(doc)]
use crate::builder::ObjectMetaBuilder;
#[cfg(doc)]
use crate::commons::product_image_selection::ResolvedProductImage;

/// Recommended labels to set on objects created by Stackable operators
///
/// See [`get_recommended_labels`] and [`ObjectMetaBuilder::with_recommended_labels`].
#[derive(Debug, Clone, Copy)]
pub struct ObjectLabels<'a, T> {
    /// The name of the object that this object is being created on behalf of, such as a `ZookeeperCluster`
    pub owner: &'a T,
    /// The name of the app being managed, such as `zookeeper`
    pub app_name: &'a str,
    /// The version of the app being managed (not of the operator).
    ///
    /// If setting this label on a Stackable product then please use [`ResolvedProductImage::app_version_label`]
    ///
    /// This version should include the Stackable version, such as `3.0.0-stackable0.1.0`.
    /// If the Stackable version is not known, then the product version should be used together with a suffix (if possible).
    /// If a custom product image is provided by the user (in which case only the product version is known),
    /// then the format `3.0.0-<tag-of-custom-image>` should be used.
    ///
    /// However, this is pure documentation and should not be parsed.
    pub app_version: &'a str,
    /// The DNS-style name of the operator managing the object (such as `zookeeper.stackable.tech`)
    pub operator_name: &'a str,
    /// The name of the controller inside of the operator managing the object (such as `zookeepercluster`)
    pub controller_name: &'a str,
    /// The role that this object belongs to
    pub role: &'a str,
    /// The role group that this object belongs to
    pub role_group: &'a str,
}

/// Create kubernetes recommended labels
pub fn get_recommended_labels<T>(
    ObjectLabels {
        owner,
        app_name,
        app_version,
        operator_name,
        controller_name,
        role,
        role_group,
    }: ObjectLabels<T>,
) -> Result<BTreeMap<String, Label>, LabelParseError>
where
    T: Resource,
{
    let mut labels = LabelListBuilder::new(Some(LABEL_KEY_PREFIX_APP_KUBERNETES));
    labels.add(
        LABEL_KEY_NAME_APP_MANAGED_BY,
        &format_full_controller_name(operator_name, controller_name),
    )?;
    labels.add(LABEL_KEY_NAME_APP_INSTANCE, &owner.name_any())?;
    labels.add(LABEL_KEY_NAME_APP_ROLE_GROUP, role_group)?;
    labels.add(LABEL_KEY_NAME_APP_VERSION, app_version)?;
    labels.add(LABEL_KEY_NAME_APP_COMPONENT, role)?;
    labels.add(LABEL_KEY_NAME_APP_NAME, app_name)?;

    // TODO: Add operator version label
    // TODO: part-of is empty for now, decide on how this can be used in a proper fashion

    Ok(labels.build())
}
