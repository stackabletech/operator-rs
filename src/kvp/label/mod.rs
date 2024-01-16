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
use std::{collections::BTreeMap, fmt::Display};

use delegate::delegate;
use kube::{Resource, ResourceExt};

use crate::{
    iter::TryFromIterator,
    kvp::{
        consts::{
            K8S_APP_COMPONENT_KEY, K8S_APP_INSTANCE_KEY, K8S_APP_MANAGED_BY_KEY, K8S_APP_NAME_KEY,
            K8S_APP_ROLE_GROUP_KEY, K8S_APP_VERSION_KEY,
        },
        Key, KeyValuePair, KeyValuePairError, KeyValuePairs, KeyValuePairsError, ObjectLabels,
    },
    utils::format_full_controller_name,
};

mod selector;
mod value;

pub use selector::*;
pub use value::*;

pub type LabelsError = KeyValuePairsError;

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
#[derive(Clone, Debug)]
pub struct Label(KeyValuePair<LabelValue>);

impl<K, V> TryFrom<(K, V)> for Label
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    type Error = LabelError;

    fn try_from(value: (K, V)) -> Result<Self, Self::Error> {
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
    ///
    /// ```
    /// # use stackable_operator::kvp::Label;
    /// let label = Label::try_from(("stackable.tech/vendor", "Stackable")).unwrap();
    /// assert_eq!(label.key().to_string(), "stackable.tech/vendor");
    /// ```
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
/// # use stackable_operator::kvp::Labels;
/// let map = BTreeMap::from([
///     ("stackable.tech/managed-by", "stackablectl"),
///     ("stackable.tech/vendor", "Stackable"),
/// ]);
///
/// let labels = Labels::try_from(map).unwrap();
/// ```
///
/// ### Creating a list of labels from an array
///
/// ```
/// # use stackable_operator::kvp::Labels;
/// let labels = Labels::try_from([
///     ("stackable.tech/managed-by", "stackablectl"),
///     ("stackable.tech/vendor", "Stackable"),
/// ]).unwrap();
/// ```
#[derive(Clone, Debug, Default)]
pub struct Labels(KeyValuePairs<LabelValue>);

impl<K, V> TryFrom<BTreeMap<K, V>> for Labels
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    type Error = LabelError;

    fn try_from(map: BTreeMap<K, V>) -> Result<Self, Self::Error> {
        Self::try_from_iter(map)
    }
}

impl<K, V> TryFrom<&BTreeMap<K, V>> for Labels
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    type Error = LabelError;

    fn try_from(map: &BTreeMap<K, V>) -> Result<Self, Self::Error> {
        Self::try_from_iter(map)
    }
}

impl<const N: usize, K, V> TryFrom<[(K, V); N]> for Labels
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    type Error = LabelError;

    fn try_from(array: [(K, V); N]) -> Result<Self, Self::Error> {
        Self::try_from_iter(array)
    }
}

impl FromIterator<KeyValuePair<LabelValue>> for Labels {
    fn from_iter<T: IntoIterator<Item = KeyValuePair<LabelValue>>>(iter: T) -> Self {
        let kvps = KeyValuePairs::from_iter(iter);
        Self(kvps)
    }
}

impl<K, V> TryFromIterator<(K, V)> for Labels
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    type Error = LabelError;

    fn try_from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Result<Self, Self::Error> {
        let kvps = KeyValuePairs::try_from_iter(iter)?;
        Ok(Self(kvps))
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
    pub fn new_with(pairs: Vec<KeyValuePair<LabelValue>>) -> Self {
        Self(KeyValuePairs::new_with(pairs))
    }

    /// Tries to insert a new label by first parsing `label` as a [`Label`]
    /// and then inserting it into the list. This function will overwrite any
    /// existing label already present.
    pub fn parse_insert(
        &mut self,
        label: impl TryInto<Label, Error = LabelError>,
    ) -> Result<(), LabelError> {
        self.0.insert(label.try_into()?.0);
        Ok(())
    }

    /// Inserts a new [`Label`]. This function will overwrite any existing label
    /// already present.
    pub fn insert(&mut self, label: Label) -> &mut Self {
        self.0.insert(label.0);
        self
    }

    /// Returns an [`Iterator`] over [`Labels`] yielding a reference to every [`Label`] contained within.
    pub fn iter(&self) -> impl Iterator<Item = &KeyValuePair<LabelValue>> {
        self.0.iter()
    }

    /// Returns a consuming [`Iterator`] over [`Labels`] moving every [`Label`] out.
    /// The [`Labels`] cannot be used again after calling this.
    pub fn into_iter(self) -> impl Iterator<Item = KeyValuePair<LabelValue>> {
        self.0.into_iter()
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

    // This forwards / delegates associated functions to the inner field. In
    // this case self.0 which is of type KeyValuePairs<T>. So calling
    // Labels::len() will be delegated to KeyValuePair<T>::len() without the
    // need to write boilerplate code.
    delegate! {
        to self.0 {
            /// Tries to insert a new [`Label`]. It ensures there are no duplicate
            /// entries. Trying to insert duplicated data returns an error. If no such
            /// check is required, use [`Labels::insert`] instead.
            pub fn try_insert(&mut self, #[newtype] label: Label) -> Result<(), LabelsError>;

            /// Extends `self` with `other`.
            pub fn extend(&mut self, #[newtype] other: Self);

            /// Returns the number of labels.
            pub fn len(&self) -> usize;

            /// Returns if the set of labels is empty.
            pub fn is_empty(&self) -> bool;

            /// Returns if the set of labels contains the provided `label`. Failure to
            /// parse/validate the [`KeyValuePair`] will return `false`.
            pub fn contains(&self, label: impl TryInto<KeyValuePair<LabelValue>>) -> bool;

            /// Returns if the set of labels contains a label with the provided `key`.
            /// Failure to parse/validate the [`Key`] will return `false`.
            pub fn contains_key(&self, key: impl TryInto<Key>) -> bool;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_insert() {
        let mut labels = Labels::new();

        labels
            .parse_insert(("stackable.tech/managed-by", "stackablectl"))
            .unwrap();

        labels
            .parse_insert(("stackable.tech/vendor", "Stackable"))
            .unwrap();

        assert_eq!(labels.len(), 2);
    }
}
