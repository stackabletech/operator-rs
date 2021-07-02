//! This module contains [`stackable_config::ConfigOption`]s and related methods to create common
//! configuration options that can be used by all operators.
use stackable_config::ConfigOption;
use std::collections::HashSet;

pub fn create_crd() -> ConfigOption {
    ConfigOption {
        name: "create-crd",
        required: false,
        takes_argument: false,
        help: "Create the CRD in the Kubernetes cluster",
        documentation: "If provided the Operator will try to create the CRD in the Kubernetes cluster. Should this operation fail the Operator will abort.",
        list: false,
        ..ConfigOption::default()
    }
}

pub fn print_crd() -> ConfigOption {
    ConfigOption {
        name: "print-crd",
        required: false,
        takes_argument: false,
        help: "Prints the YAML CRD to stdout",
        documentation: "If provided the Operator will print the CRD to stdout and exit.",
        list: false,
        ..ConfigOption::default()
    }
}

pub fn operator_options() -> HashSet<ConfigOption> {
    let mut options = HashSet::new();
    options.insert(create_crd());
    options.insert(print_crd());
    options
}
