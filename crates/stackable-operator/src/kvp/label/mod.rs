//! This module provides various types and functions to construct valid
//! Kubernetes labels. Labels are key/value pairs, where the key must meet
//! certain requirementens regarding length and character set. The value can
//! contain a limited set of ASCII characters.
//!
//! Additionally, the [`Label`] struct provides various helper functions to
//! construct commonly used labels across the Stackable Data Platform, like
//! the role_group or component.
//!
//! See <https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/>
//! for more information on Kubernetes labels.

use crate::kvp::{KeyValuePair, KeyValuePairError, KeyValuePairs};

mod selector;
mod value;

pub use selector::*;
pub use value::*;

/// A type alias for errors returned when construction or manipulation of a set
/// of labels fails.
pub type LabelError = KeyValuePairError<LabelValueError>;

/// A specialized implementation of a key/value pair representing Kubernetes
/// labels.
///
/// ```
/// # use stackable_operator::kvp::Label;
/// let label = Label::try_from(("stackable.tech/vendor", "Stackable")).unwrap();
/// assert_eq!(label.to_string(), "stackable.tech/vendor=Stackable");
/// ```
///
/// The validation of the label value can fail due to multiple reasons. It can
/// only contain a limited set and combination of ASCII characters. See
/// <https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/>
/// for more information on Kubernetes labels.
pub type Label = KeyValuePair<LabelValue>;

/// A validated set/list of Kubernetes labels.
///
/// It provides selected associated functions to manipulate the set of labels,
/// like inserting or extending.
///
/// ## Examples
///
/// ### Converting a BTreeMap into a list of labels
///
/// ```
/// # use std::collections::BTreeMap;
/// # use stackable_operator::iter::TryFromIterator;
/// # use stackable_operator::kvp::Labels;
/// let map = BTreeMap::from([
///     ("stackable.tech/managed-by", "stackablectl"),
///     ("stackable.tech/vendor", "Stackable"),
/// ]);
///
/// let labels = Labels::try_from_iter(map).unwrap();
/// ```
///
/// ### Creating a list of labels from an array
///
/// ```
/// # use stackable_operator::iter::TryFromIterator;
/// # use stackable_operator::kvp::Labels;
/// let labels = Labels::try_from_iter([
///     ("stackable.tech/managed-by", "stackablectl"),
///     ("stackable.tech/vendor", "Stackable"),
/// ]).unwrap();
/// ```
pub type Labels = KeyValuePairs<LabelValue>;

/// Well-known labels used by other tools or standard conventions.
pub mod well_known {
    use crate::{
        kvp::consts::{
            K8S_APP_COMPONENT_KEY, K8S_APP_MANAGED_BY_KEY, K8S_APP_ROLE_GROUP_KEY,
            K8S_APP_VERSION_KEY, STACKABLE_VENDOR_KEY, STACKABLE_VENDOR_VALUE,
        },
        utils::format_full_controller_name,
    };

    use super::{Label, LabelError};

    /// Creates the `app.kubernetes.io/component` label with `role` as the
    /// value. This function will return an error if `role` violates the required
    /// Kubernetes restrictions.
    pub fn component(component: &str) -> Result<Label, LabelError> {
        Label::try_from((K8S_APP_COMPONENT_KEY, component))
    }

    /// Creates the `app.kubernetes.io/role-group` label with `role_group` as
    /// the value. This function will return an error if `role_group` violates
    /// the required Kubernetes restrictions.
    pub fn role_group(role_group: &str) -> Result<Label, LabelError> {
        Label::try_from((K8S_APP_ROLE_GROUP_KEY, role_group))
    }

    /// Creates the `app.kubernetes.io/managed-by` label with the formated
    /// full controller name based on `operator_name` and `controller_name` as
    /// the value. This function will return an error if the formatted controller
    /// name violates the required Kubernetes restrictions.
    pub fn managed_by(operator_name: &str, controller_name: &str) -> Result<Label, LabelError> {
        Label::try_from((
            K8S_APP_MANAGED_BY_KEY,
            format_full_controller_name(operator_name, controller_name).as_str(),
        ))
    }

    /// Creates the `app.kubernetes.io/version` label with `version` as the
    /// value. This function will return an error if `role_group` violates the
    /// required Kubernetes restrictions.
    pub fn version(version: &str) -> Result<Label, LabelError> {
        Label::try_from((K8S_APP_VERSION_KEY, version))
    }

    pub fn vendor_stackable() -> Label {
        Label::try_from((STACKABLE_VENDOR_KEY, STACKABLE_VENDOR_VALUE))
            .expect("failed to parse hard-coded Stackable vendor label")
    }
}

/// Common sets of labels that apply for different use-cases.
pub mod sets {
    use kube::{Resource, ResourceExt};

    use crate::kvp::{
        consts::{K8S_APP_INSTANCE_KEY, K8S_APP_NAME_KEY},
        ObjectLabels,
    };

    use super::{well_known, Label, LabelError, Labels};

    /// Returns the recommended set of labels. The set includes these well-known
    /// Kubernetes labels:
    ///
    /// - `app.kubernetes.io/role-group`
    /// - `app.kubernetes.io/managed-by`
    /// - `app.kubernetes.io/component`
    /// - `app.kubernetes.io/instance`
    /// - `app.kubernetes.io/version`
    /// - `app.kubernetes.io/name`
    ///
    /// Additionally, it includes Stackable-specific labels. These are:
    ///
    /// - `stackable.tech/vendor`
    ///
    /// This function returns a result, because the parameter `object_labels`
    /// can contain invalid data or can exceed the maximum allowed number of
    /// characters.
    pub fn recommended<R>(object_labels: ObjectLabels<R>) -> Result<Labels, LabelError>
    where
        R: Resource,
    {
        // Well-known Kubernetes labels
        let mut labels = role_group_selector(
            object_labels.owner,
            object_labels.app_name,
            object_labels.role,
            object_labels.role_group,
        )?;

        labels.extend([
            well_known::managed_by(object_labels.operator_name, object_labels.controller_name)?,
            well_known::version(object_labels.app_version)?,
            // Stackable-specific labels
            well_known::vendor_stackable(),
        ]);

        Ok(labels)
    }

    /// Returns the set of labels required to select the resource based on the
    /// role group. The set contains role selector labels, see
    /// [`role_selector`] for more details. Additionally, it contains
    /// the `app.kubernetes.io/role-group` label with `role_group` as the value.
    pub fn role_group_selector<R>(
        owner: &R,
        app_name: &str,
        role: &str,
        role_group: &str,
    ) -> Result<Labels, LabelError>
    where
        R: Resource,
    {
        let mut labels = role_selector(owner, app_name, role)?;
        labels.extend([well_known::role_group(role_group)?]);
        Ok(labels)
    }

    /// Returns the set of labels required to select the resource based on the
    /// role. The set contains the common labels, see [`common`] for
    /// more details. Additionally, it contains the `app.kubernetes.io/component`
    /// label with `role` as the value.
    ///
    /// This function returns a result, because the parameters `owner`, `app_name`,
    /// and `role` can contain invalid data or can exceed the maximum allowed
    /// number fo characters.
    pub fn role_selector<R>(owner: &R, app_name: &str, role: &str) -> Result<Labels, LabelError>
    where
        R: Resource,
    {
        let mut labels = common(app_name, owner.name_any().as_str())?;
        labels.extend([well_known::component(role)?]);
        Ok(labels)
    }

    /// Returns a common set of labels, which are required to identify resources
    /// that belong to a certain owner object, for example a `ZookeeperCluster`.
    /// The set contains these well-known labels:
    ///
    /// - `app.kubernetes.io/instance` and
    /// - `app.kubernetes.io/name`
    ///
    /// This function returns a result, because the parameters `app_name` and
    /// `app_instance` can contain invalid data or can exceed the maximum
    /// allowed number of characters.
    pub fn common(app_name: &str, app_instance: &str) -> Result<Labels, LabelError> {
        Ok(Labels::from_iter([
            Label::try_from((K8S_APP_INSTANCE_KEY, app_instance))?,
            Label::try_from((K8S_APP_NAME_KEY, app_name))?,
        ]))
    }
}
