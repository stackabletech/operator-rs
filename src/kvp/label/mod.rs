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
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
};

use kube::{Resource, ResourceExt};
use serde::{Deserialize, Serialize};

use crate::{
    kvp::{
        consts::{
            K8S_APP_COMPONENT_KEY, K8S_APP_INSTANCE_KEY, K8S_APP_MANAGED_BY_KEY, K8S_APP_NAME_KEY,
            K8S_APP_ROLE_GROUP_KEY, K8S_APP_VERSION_KEY,
        },
        Key, KeyValuePair, KeyValuePairError, KeyValuePairs, KeyValuePairsError, ObjectLabels,
    },
    utils::format_full_controller_name,
};

mod value;

pub use value::*;

/// This is an type alias for [`KeyValuePairsError<LabelValueError>`]. This
/// error is returned when an error occurs while manipulating [`Labels`].
pub type LabelsError = KeyValuePairsError<LabelValueError>;

/// This is an type alias for [`KeyValuePairError<LabelValueError>`]. This
/// error is returned when constructing a [`Label`].
pub type LabelError = KeyValuePairError<LabelValueError>;

/// [`Label`] is a specialized implementation of [`KeyValuePair`]. The
/// validation of the label value can fail due to multiple reasons. It can only
/// contain a limited set of ASCII characters.
///
/// See <https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/>
/// for more information on Kubernetes labels.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Label(KeyValuePair<LabelValue>);

impl<T> TryFrom<(T, T)> for Label
where
    T: AsRef<str>,
{
    type Error = LabelError;

    fn try_from(value: (T, T)) -> Result<Self, Self::Error> {
        let kvp = KeyValuePair::try_from(value)?;
        Ok(Self(kvp))
    }
}

impl Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Label {
    /// Returns an immutable reference to the label's [`Key`].
    pub fn key(&self) -> &Key {
        self.0.key()
    }

    /// Returns an immutable reference to the label's value.
    pub fn value(&self) -> &LabelValue {
        self.0.value()
    }

    /// Consumes self and returns the inner [`KeyValuePair<LabelValue>`].
    pub fn into_inner(self) -> KeyValuePair<LabelValue> {
        self.0
    }

    /// Creates the `app.kubernetes.io/component` label with `role` as the
    /// value. This function will return an error if `role` violates the required
    /// Kubernetes restrictions.
    pub fn component(component: &str) -> Result<Self, LabelError> {
        let kvp = KeyValuePair::try_from((K8S_APP_COMPONENT_KEY, component))?;
        Ok(Self(kvp))
    }

    /// Creates the `app.kubernetes.io/role-group` label with `role_group` as
    /// the value. This function will return an error if `role_group` violates
    /// the required Kubernetes restrictions.
    pub fn role_group(role_group: &str) -> Result<Self, LabelError> {
        let kvp = KeyValuePair::try_from((K8S_APP_ROLE_GROUP_KEY, role_group))?;
        Ok(Self(kvp))
    }

    /// Creates the `app.kubernetes.io/managed-by` label with the formated
    /// full controller name based on `operator_name` and `controller_name` as
    /// the value. This function will return an error if the formatted controller
    /// name violates the required Kubernetes restrictions.
    pub fn managed_by(operator_name: &str, controller_name: &str) -> Result<Self, LabelError> {
        let kvp = KeyValuePair::try_from((
            K8S_APP_MANAGED_BY_KEY,
            format_full_controller_name(operator_name, controller_name).as_str(),
        ))?;
        Ok(Self(kvp))
    }

    /// Creates the `app.kubernetes.io/version` label with `version` as the
    /// value. This function will return an error if `role_group` violates the
    /// required Kubernetes restrictions.
    pub fn version(version: &str) -> Result<Self, LabelError> {
        // NOTE (Techassi): Maybe use semver::Version
        let kvp = KeyValuePair::try_from((K8S_APP_VERSION_KEY, version))?;
        Ok(Self(kvp))
    }
}

/// [`Labels`] is a set of [`Label`]. It provides selected associated functions
/// to manipulate the set of labels, like inserting or extending.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Labels(KeyValuePairs<LabelValue>);

impl TryFrom<BTreeMap<String, String>> for Labels {
    type Error = LabelError;

    fn try_from(value: BTreeMap<String, String>) -> Result<Self, Self::Error> {
        let kvps = KeyValuePairs::try_from(value)?;
        Ok(Self(kvps))
    }
}

impl FromIterator<KeyValuePair<LabelValue>> for Labels {
    fn from_iter<T: IntoIterator<Item = KeyValuePair<LabelValue>>>(iter: T) -> Self {
        let kvps = KeyValuePairs::from_iter(iter);
        Self(kvps)
    }
}

impl From<Labels> for BTreeMap<String, String> {
    fn from(value: Labels) -> Self {
        value.0.into()
    }
}

impl Labels {
    /// Creates a new empty list of [`Labels`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new list of [`Labels`] from `pairs`.
    pub fn new_with(pairs: BTreeSet<KeyValuePair<LabelValue>>) -> Self {
        Self(KeyValuePairs::new_with(pairs))
    }

    /// Tries to insert a new [`Label`]. It ensures there are no duplicate
    /// entries. Trying to insert duplicated data returns an error. If no such
    /// check is required, use the `insert` function instead.
    pub fn try_insert(&mut self, label: Label) -> Result<&mut Self, LabelsError> {
        self.0.try_insert(label.0)?;
        Ok(self)
    }

    /// Inserts a new [`Label`]. This function will overide any existing label
    /// already present. If this behaviour is not desired, use the `try_insert`
    /// function instead.
    pub fn insert(&mut self, label: Label) -> &mut Self {
        self.0.insert(label.0);
        self
    }

    /// Extends `self` with `other`.
    pub fn extend(&mut self, other: Self) {
        self.0.extend(other.0)
    }

    /// Returns the number of labels.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns if the set of labels is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the recommended set of labels. The set includes these well-known
    /// labels:
    ///
    /// - `app.kubernetes.io/role-group`
    /// - `app.kubernetes.io/managed-by`
    /// - `app.kubernetes.io/component`
    /// - `app.kubernetes.io/instance`
    /// - `app.kubernetes.io/version`
    /// - `app.kubernetes.io/name`
    ///
    /// This function returns a result, because the parameter `object_labels`
    /// can contain invalid data or can exceed the maximum allowed number of
    /// characters.
    pub fn recommended<R>(object_labels: ObjectLabels<R>) -> Result<Self, LabelError>
    where
        R: Resource,
    {
        let mut labels = Self::role_group_selector(
            object_labels.owner,
            object_labels.app_name,
            object_labels.role,
            object_labels.role_group,
        )?;

        let managed_by =
            Label::managed_by(object_labels.operator_name, object_labels.controller_name)?;
        let version = Label::version(object_labels.app_version)?;

        labels.insert(managed_by);
        labels.insert(version);

        Ok(labels)
    }

    /// Returns the set of labels required to select the resource based on the
    /// role group. The set contains role selector labels, see
    /// [`Labels::role_selector`] for more details. Additionally, it contains
    /// the `app.kubernetes.io/role-group` label with `role_group` as the value.
    pub fn role_group_selector<R>(
        owner: &R,
        app_name: &str,
        role: &str,
        role_group: &str,
    ) -> Result<Self, LabelError>
    where
        R: Resource,
    {
        let mut labels = Self::role_selector(owner, app_name, role)?;
        labels.insert(Label::role_group(role_group)?);
        Ok(labels)
    }

    /// Returns the set of labels required to select the resource based on the
    /// role. The set contains the common labels, see [`Labels::common`] for
    /// more details. Additionally, it contains the `app.kubernetes.io/component`
    /// label with `role` as the value.
    ///
    /// This function returns a result, because the parameters `owner`, `app_name`,
    /// and `role` can contain invalid data or can exceed the maximum allowed
    /// number fo characters.
    pub fn role_selector<R>(owner: &R, app_name: &str, role: &str) -> Result<Self, LabelError>
    where
        R: Resource,
    {
        let mut labels = Self::common(app_name, owner.name_any().as_str())?;
        labels.insert(Label::component(role)?);
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
    pub fn common(app_name: &str, app_instance: &str) -> Result<Self, LabelError> {
        let mut labels = Self::new();

        labels.insert((K8S_APP_INSTANCE_KEY, app_instance).try_into()?);
        labels.insert((K8S_APP_NAME_KEY, app_name).try_into()?);

        Ok(labels)
    }
}
