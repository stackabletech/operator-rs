//! This module provides helpers and constants to deal with namespaces
use crate::client::Client;
use k8s_openapi::NamespaceResourceScope;
use kube::{Api, Resource};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WatchNamespace {
    All,
    One(String),
}

impl From<&str> for WatchNamespace {
    fn from(s: &str) -> Self {
        if s.is_empty() {
            WatchNamespace::All
        } else {
            WatchNamespace::One(s.to_string())
        }
    }
}

impl WatchNamespace {
    /// Gets an API object for the namespace in question or for all namespaces,
    /// depending on which variant we are.
    pub fn get_api<T>(&self, client: &Client) -> Api<T>
    where
        T: Resource<DynamicType = (), Scope = NamespaceResourceScope>,
    {
        match self {
            WatchNamespace::All => client.get_all_api(),
            WatchNamespace::One(namespace) => client.get_api::<T>(namespace),
        }
    }
}
