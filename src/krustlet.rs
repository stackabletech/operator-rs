use k8s_openapi::api::core::v1::Toleration;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use std::collections::BTreeMap;

/// Creates a vector of tolerations we need to work with the Krustlet.
/// Usually these would be added to a Pod so it can be scheduled on a Krustlet.
pub fn create_tolerations() -> Vec<Toleration> {
    vec![
        Toleration {
            effect: Some(String::from("NoExecute")),
            key: Some(String::from("kubernetes.io/arch")),
            operator: Some(String::from("Equal")),
            toleration_seconds: None,
            value: Some(String::from("stackable-linux")),
        },
        Toleration {
            effect: Some(String::from("NoSchedule")),
            key: Some(String::from("kubernetes.io/arch")),
            operator: Some(String::from("Equal")),
            toleration_seconds: None,
            value: Some(String::from("stackable-linux")),
        },
        Toleration {
            effect: Some(String::from("NoSchedule")),
            key: Some(String::from("node.kubernetes.io/network-unavailable")),
            operator: Some(String::from("Exists")),
            toleration_seconds: None,
            value: None,
        },
    ]
}

/// Helper method to make sure that any LabelSelector we use only matches our own "special" nodes.
/// At the moment this label is "type" with the value "krustlet" and we'll use match_labels.
///
/// WARN: Should a label "type" already be used this will be overridden!
/// If this is really needed add a match_expression
///
/// We will not however change the original LabelSelector, a new one will be returned.
pub fn add_stackable_selector(selector: Option<&LabelSelector>) -> LabelSelector {
    let mut selector = match selector {
        None => LabelSelector::default(),
        Some(selector) => selector.clone(),
    };

    selector
        .match_labels
        .get_or_insert_with(BTreeMap::new)
        .insert("type".to_string(), "krustlet".to_string());
    selector
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
    use std::collections::BTreeMap;

    #[test]
    fn test_add_stackable_selector() {
        let mut ls = LabelSelector {
            match_expressions: None,
            match_labels: None,
        };

        // LS didn't have any match_label
        assert!(
            matches!(add_stackable_selector(Some(&ls)).match_labels, Some(labels) if labels.get("type").unwrap() == "krustlet")
        );

        // LS has labels but no conflicts with our own
        let mut labels = BTreeMap::new();
        labels.insert("foo".to_string(), "bar".to_string());

        ls.match_labels = Some(labels);
        assert!(
            matches!(add_stackable_selector(Some(&ls)).match_labels, Some(labels) if labels.get("type").unwrap() == "krustlet")
        );

        // LS already has a LS that matches our internal one
        let mut labels = BTreeMap::new();
        labels.insert("foo".to_string(), "bar".to_string());
        labels.insert("type".to_string(), "foobar".to_string());
        ls.match_labels = Some(labels);
        assert!(
            matches!(add_stackable_selector(Some(&ls)).match_labels, Some(labels) if labels.get("type").unwrap() == "krustlet")
        );
    }
}
