use stackable_config::ConfigOption;

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
