//! This module provides various types and functions to construct valid Kubernetes labels. Labels
//! are key/value pairs, where the key must meet certain requirements regarding length and character
//! set. The value can contain a limited set of ASCII characters.
//!
//! Additionally, the [`Label`] struct provides various helper functions to construct commonly used
//! labels across the Stackable Data Platform, like the `role_group` or `component`.
//!
//! See <https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/> for more
//! information on Kubernetes labels.

use crate::kvp::{KeyValuePair, KeyValuePairError, KeyValuePairs};

mod selector;
mod value;

pub use selector::*;
pub use value::*;

/// A type alias for errors returned when construction or manipulation of a set of labels fails.
pub type LabelError = KeyValuePairError<LabelValueError>;

/// A specialized implementation of a key/value pair representing Kubernetes labels.
///
/// ```
/// # use stackable_operator::kvp::Label;
/// let label = Label::try_from(("stackable.tech/vendor", "Stackable")).unwrap();
/// assert_eq!(label.to_string(), "stackable.tech/vendor=Stackable");
/// ```
///
/// The validation of the label value can fail due to multiple reasons. It can only contain a
/// limited set and combination of ASCII characters. See [the documentation][1] for more information
/// on Kubernetes labels.
///
/// [1]: https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/
pub type Label = KeyValuePair<LabelValue>;

/// A validated set/list of Kubernetes labels.
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
    use crate::utils::format_full_controller_name;

    use super::{Label, LabelError, Labels};

    /// Creates the `app.kubernetes.io/component` label.
    ///
    /// It is used to specify the component within the architecture, e.g. `database`.
    ///
    /// This function will return an error if `role` violates the required Kubernetes restrictions.
    pub fn component(component: &str) -> Result<Label, LabelError> {
        Label::try_from(("app.kubernetes.io/component", component))
    }

    /// Creates the `app.kubernetes.io/managed-by` label with the formatted full controller name as
    /// a value.
    ///
    /// The controller name is based on `operator_name` and `controller_name`.  It is used to
    /// indicate what tool is being used to manage the operation of an application, e.g. `helm`.
    ///
    /// This function will return an error if the formatted controller name violates the required
    /// Kubernetes restrictions.
    pub fn managed_by(operator_name: &str, controller_name: &str) -> Result<Label, LabelError> {
        Label::try_from((
            "app.kubernetes.io/managed-by",
            format_full_controller_name(operator_name, controller_name).as_str(),
        ))
    }

    /// Creates the `app.kubernetes.io/version` label.
    ///
    /// It is used to indicate the current version of the application. The value can represent a
    /// semantic version or a revision, e.g. `5.7.21`.
    ///
    /// This function will return an error if `role_group` violates the required Kubernetes
    /// restrictions.
    pub fn version(version: &str) -> Result<Label, LabelError> {
        Label::try_from(("app.kubernetes.io/version", version))
    }

    /// Creates the `stackable.tech/vendor: Stackable` label, tagging the object as created by a
    /// Stackable operator.
    pub fn vendor_stackable() -> Label {
        Label::try_from(("stackable.tech/vendor", "Stackable"))
            .expect("failed to parse hard-coded Stackable vendor label")
    }

    /// Creates the `stackable.tech/role-group` label.
    ///
    /// This function will return an error if `role_group` violates the required Kubernetes
    /// restrictions.
    pub fn role_group(role_group: &str) -> Result<Label, LabelError> {
        Label::try_from(("stackable.tech/role-group", role_group))
    }

    /// Common sets of labels that apply for different use-cases.
    pub mod sets {
        use kube::{Resource, ResourceExt};

        use crate::kvp::ObjectLabels;

        use super::{Label, LabelError, Labels};

        /// Returns the recommended set of labels.
        ///
        /// The set includes these well-known Kubernetes labels:
        ///
        /// - `app.kubernetes.io/managed-by`
        /// - `app.kubernetes.io/component`
        /// - `app.kubernetes.io/instance`
        /// - `app.kubernetes.io/version`
        /// - `app.kubernetes.io/name`
        ///
        /// Additionally, it includes these Stackable-specific labels:
        ///
        /// - `stackable.tech/role-group`
        /// - `stackable.tech/vendor`
        ///
        /// This function returns a [`Result`], because the parameter `object_labels` can contain
        /// invalid data or can exceed the maximum allowed number of characters.
        pub fn recommended<R>(object_labels: ObjectLabels<R>) -> Result<Labels, LabelError>
        where
            R: Resource,
        {
            let mut labels = role_group_selector(
                object_labels.owner,
                object_labels.app_name,
                object_labels.role,
                object_labels.role_group,
            )?;

            labels.extend([
                super::managed_by(object_labels.operator_name, object_labels.controller_name)?,
                super::version(object_labels.app_version)?,
                // Stackable-specific labels
                super::vendor_stackable(),
            ]);

            Ok(labels)
        }

        /// Returns the set of labels required to select the resource based on the role group.
        ///
        /// The set contains role selector labels, see [`role_selector`] for more details.
        /// Additionally, it contains the `stackable.tech/role-group` label with `role_group` as the
        /// value.
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
            labels.extend([super::role_group(role_group)?]);
            Ok(labels)
        }

        /// Returns the set of labels required to select the resource based on the role.
        ///
        /// The set contains the common labels, see [`common`] for more details. Additionally, it
        /// contains the `app.kubernetes.io/component` label with `role` as the value.
        ///
        /// This function returns a result, because the parameters `owner`, `app_name`,
        /// and `role` can contain invalid data or can exceed the maximum allowed
        /// number fo characters.
        pub fn role_selector<R>(owner: &R, app_name: &str, role: &str) -> Result<Labels, LabelError>
        where
            R: Resource,
        {
            let mut labels = common(app_name, owner.name_any().as_str())?;
            labels.extend([super::component(role)?]);
            Ok(labels)
        }

        /// Returns a common set of labels, which are required to identify resources that belong to
        /// a certain owner object, for example a `ZookeeperCluster`.
        ///
        /// The set contains these well-known labels:
        ///
        /// - `app.kubernetes.io/instance` and
        /// - `app.kubernetes.io/name`
        ///
        /// This function returns a result, because the parameters `app_name` and `app_instance` can
        /// contain invalid data or can exceed the maximum allowed number of characters.
        pub fn common(app_name: &str, app_instance: &str) -> Result<Labels, LabelError> {
            Ok(Labels::from_iter([
                Label::try_from(("app.kubernetes.io/instance", app_instance))?,
                Label::try_from(("app.kubernetes.io/name", app_name))?,
            ]))
        }
    }
}
