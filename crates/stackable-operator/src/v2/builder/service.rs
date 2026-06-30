use crate::{
    kvp::{Annotations, Labels},
    v2::types::common::Port,
};

pub enum Scraping {
    Enabled,
    Disabled,
}

pub enum Scheme {
    Http,
    Https,
}

/// Common labels for Prometheus
pub fn prometheus_labels(scraping_enabled: &Scraping) -> Labels {
    Labels::try_from([(
        "prometheus.io/scrape",
        // "true" and "false" are valid label values.
        match scraping_enabled {
            Scraping::Enabled => "true".to_owned(),
            Scraping::Disabled => "false".to_owned(),
        },
    )])
    .expect("should be a valid label")
}

/// Common annotations for Prometheus
///
/// These annotations can be used in a ServiceMonitor.
///
/// see also <https://github.com/prometheus-community/helm-charts/blob/prometheus-27.32.0/charts/prometheus/values.yaml#L983-L1036>
///
/// # Example
///
/// ```rust
/// # use stackable_operator::v2::{
/// #     builder::service::{Scheme, Scraping, prometheus_annotations},
/// #     types::common::Port,
/// # };
///
/// prometheus_annotations(
///     &Scraping::Enabled,
///     &Scheme::Https,
///     "/_prometheus/metrics",
///     &Port(9200),
/// );
/// ```
pub fn prometheus_annotations(
    scraping_enabled: &Scraping,
    scheme: &Scheme,
    path: &str,
    port: &Port,
) -> Annotations {
    // There are no restrictions on annotation values, so it is not necessary to check the given
    // parameters.
    Annotations::try_from([
        ("prometheus.io/path".to_owned(), path.to_owned()),
        ("prometheus.io/port".to_owned(), port.to_string()),
        (
            "prometheus.io/scheme".to_owned(),
            match scheme {
                Scheme::Http => "http".to_owned(),
                Scheme::Https => "https".to_owned(),
            },
        ),
        (
            "prometheus.io/scrape".to_owned(),
            match scraping_enabled {
                Scraping::Enabled => "true".to_owned(),
                Scraping::Disabled => "false".to_owned(),
            },
        ),
    ])
    .expect("should be valid annotations")
}
