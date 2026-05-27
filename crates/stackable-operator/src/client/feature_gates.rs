use std::{collections::HashMap, str::FromStr};

use snafu::{OptionExt as _, ResultExt as _, Snafu};
use winnow::{
    Parser as _,
    ascii::{alphanumeric0, alphanumeric1, digit1, space0, space1},
    combinator::{delimited, separated, separated_pair},
};

use crate::client::{
    Client, CreateRawRequestSnafu, ParseFeatureGateSnafu, PerformRawRequestSnafu, Result,
};

impl Client {
    /// Retrieves and parses all feature gates via a raw request to the `/metrics` endpoint.
    ///
    /// This list of feature gates in combination with [`kube::Client::apiserver_version`] can be used
    /// to enable gated behaviour.
    pub async fn get_feature_gates(&self) -> Result<Vec<FeatureGate>> {
        let request =
            http::Request::get("/metrics")
                .body(vec![])
                .context(CreateRawRequestSnafu {
                    method: http::Method::GET,
                })?;

        let response = self
            .client
            .request_text(request)
            .await
            .context(PerformRawRequestSnafu)?;

        FeatureGate::parse_from_metrics(&response)
    }

    /// Retrieves enabled feature gates.
    ///
    /// Uses [`Client::get_feature_gates`] internally.
    pub async fn get_enabled_feature_gates(&self) -> Result<Vec<FeatureGate>> {
        let feature_gates = self.get_feature_gates().await?;
        let enabled_feature_gates = feature_gates.into_iter().filter(|fg| fg.enabled).collect();

        Ok(enabled_feature_gates)
    }

    /// Retrieves disabled feature gates.
    ///
    /// Uses [`Client::get_feature_gates`] internally.
    pub async fn get_disabled_feature_gates(&self) -> Result<Vec<FeatureGate>> {
        let feature_gates = self.get_feature_gates().await?;
        let disabled_feature_gates = feature_gates.into_iter().filter(|fg| !fg.enabled).collect();

        Ok(disabled_feature_gates)
    }
}

#[derive(Debug, Snafu)]
enum FeatureGateParseError {
    #[snafu(display("required feature gate metric label missing, expected 'name' and 'stage'"))]
    MissingLabel,

    #[snafu(display("failed to parse feature stage"))]
    ParseStage { source: strum::ParseError },

    #[snafu(display("failed to parse string as integer"))]
    ParseInt { source: std::num::ParseIntError },
}

#[derive(Debug)]
pub struct FeatureGate {
    /// The name of the feature gate, eg. `AllowDNSOnlyNodeCSR`.
    pub name: String,

    /// In which stage the feature is, eg. `ALPHA`.
    pub stage: FeatureStage,

    /// Whether the feature is enabled or disabled.
    pub enabled: bool,
}

impl FromStr for FeatureGate {
    type Err = String;

    fn from_str(s: &str) -> std::prelude::v1::Result<Self, Self::Err> {
        Self::parse_from_metric
            .parse(s)
            .map_err(|err| err.to_string())
    }
}

impl FeatureGate {
    pub const METRIC_NAME: &str = "kubernetes_feature_enabled";

    /// Enumerates the complete body line-by-line and parses the relevant feature gate metrics.
    #[allow(clippy::result_large_err)]
    fn parse_from_metrics(body: &str) -> Result<Vec<Self>> {
        body.lines()
            .filter(|l| l.starts_with(Self::METRIC_NAME))
            .map(Self::from_str)
            .collect::<Result<Vec<Self>, _>>()
            .map_err(|error| ParseFeatureGateSnafu { error }.build())
    }

    /// Parses a feature gate from the line-based `/metrics` response.
    ///
    /// This function expects feature gates to be passed as individual lines.
    fn parse_from_metric(input: &mut &str) -> winnow::Result<Self> {
        (
            Self::parse_metric_name,
            delimited('{', Self::parse_labels, '}'),
            // At least one space after the metric and the value
            space1,
            // The counter value
            digit1,
        )
            .try_map(|((), mut kv_pairs, _, count)| {
                let name = kv_pairs
                    .remove("name")
                    .context(MissingLabelSnafu)?
                    .to_owned();

                let stage = kv_pairs
                    .remove("stage")
                    .context(MissingLabelSnafu)?
                    .parse()
                    .context(ParseStageSnafu)?;

                let count = count.parse::<u8>().context(ParseIntSnafu)?;
                // TODO (@Techassi): Potentially replace this with TryFrom instead.
                // The TryFrom<u8> impl for bool is only available in Rust 1.95+
                let enabled = count != 0;

                Ok::<Self, FeatureGateParseError>(Self {
                    name,
                    stage,
                    enabled,
                })
            })
            .parse_next(input)
    }

    /// Parses (and removes) the well-known, static metric name.
    fn parse_metric_name(input: &mut &str) -> winnow::Result<()> {
        Self::METRIC_NAME.void().parse_next(input)
    }

    /// Parses and collects a list of labels contained within `{` and `}`.
    fn parse_labels<'s>(input: &mut &'s str) -> winnow::Result<HashMap<&'s str, &'s str>> {
        separated(
            // We expect at least two labels: name and stage
            2..,
            // The value of the label can be empty
            separated_pair(alphanumeric1, '=', ('"', alphanumeric0, '"'))
                .map(|(key, (_, value, _))| (key, value)),
            // There might be spaces between labels (separated by comma)
            (',', space0),
        )
        .parse_next(input)
    }
}

/// A feature can be in one of four different stages.
///
/// See the [list of feature gates] and [feature stages] in the official documentation.
///
/// [list of feature gates]: https://v1-35.docs.kubernetes.io/docs/reference/command-line-tools-reference/feature-gates/#feature-gates
/// [feature stages]: https://v1-35.docs.kubernetes.io/docs/reference/command-line-tools-reference/feature-gates/#feature-stages
#[derive(Debug, strum::Display, strum::EnumString)]
#[strum(serialize_all = "UPPERCASE")]
pub enum FeatureStage {
    /// An Alpha feature.
    ///
    /// - Disabled by default.
    /// - Might be buggy. Enabling the feature may expose bugs.
    /// - Support for feature may be dropped at any time without notice.
    /// - The API may change in incompatible ways in a later software release without notice.
    /// - Recommended for use only in short-lived testing clusters, due to increased risk of bugs
    ///   and lack of long-term support.
    ///
    /// Taken from the Kubernetes documentation.
    Alpha,

    /// A Beta feature.
    ///
    /// - Usually enabled by default. Beta API groups are [disabled by default].
    /// - The feature is well tested. Enabling the feature is considered safe.
    /// - Support for the overall feature will not be dropped, though details may change.
    /// - The schema and/or semantics of objects may change in incompatible ways in a subsequent
    ///   beta or stable release. When this happens, we will provide instructions for migrating to
    ///   the next version. This may require deleting, editing, and re-creating API objects. The
    ///   editing process may require some thought. This may require downtime for applications that
    ///   rely on the feature.
    /// - Recommended for only non-business-critical uses because of potential for incompatible
    ///   changes in subsequent releases. If you have multiple clusters that can be upgraded
    ///   independently, you may be able to relax this restriction.
    ///
    /// Taken from the Kubernetes documentation.
    Beta,

    /// A General Availability feature.
    ///
    /// - The feature is always enabled; you cannot disable it.
    /// - The corresponding feature gate is no longer needed.
    /// - Stable versions of features will appear in released software for many subsequent versions.
    ///
    /// Taken from the Kubernetes documentation.
    #[strum(serialize = "")]
    GeneralAvailability,

    /// A feature is deprecated.
    ///
    /// The official documentation doesn't explain this stage at all, but it exists (in metrics).
    Deprecated,
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use rstest::rstest;

    use super::*;
    use crate::client::{initialize_operator, tests::test_cluster_info_opts};

    #[tokio::test]
    #[ignore = "Tests depending on Kubernetes are not ran by default"]
    async fn k8s_test_feature_gates() {
        let client = initialize_operator(None, &test_cluster_info_opts())
            .await
            .expect("KUBECONFIG variable must be configured.");

        let feature_gates = client
            .get_feature_gates()
            .await
            .expect("list of feature gates must parse");

        for feature_gate in feature_gates {
            println!("{feature_gate:?}");
        }
    }

    #[test]
    fn parse_feature_gates() {
        // This snippet is a combination of
        //
        // - kubectl get --raw /metrics | head
        // - kubectl get --raw /metrics | grep kubernetes_feature_enabled | head
        let response = indoc! {r#"
            # HELP aggregator_discovery_aggregation_count_total [ALPHA] Counter of number of times discovery was aggregated
            # TYPE aggregator_discovery_aggregation_count_total counter
            aggregator_discovery_aggregation_count_total 614
            # HELP aggregator_unavailable_apiservice [ALPHA] Gauge of APIServices which are marked as unavailable broken down by APIService name.
            # TYPE aggregator_unavailable_apiservice gauge
            aggregator_unavailable_apiservice{name="v1."} 0
            aggregator_unavailable_apiservice{name="v1.admissionregistration.k8s.io"} 0
            aggregator_unavailable_apiservice{name="v1.apiextensions.k8s.io"} 0
            aggregator_unavailable_apiservice{name="v1.apps"} 0
            aggregator_unavailable_apiservice{name="v1.authentication.k8s.io"} 0
            # ...
            # HELP kubernetes_feature_enabled [BETA] This metric records the data about the stage and enablement of a k8s feature.
            # TYPE kubernetes_feature_enabled gauge
            kubernetes_feature_enabled{name="APIResponseCompression",stage="BETA"} 1
            kubernetes_feature_enabled{name="APIServerIdentity",stage="BETA"} 1
            kubernetes_feature_enabled{name="APIServerTracing",stage="BETA"} 1
            kubernetes_feature_enabled{name="APIServingWithRoutine",stage="ALPHA"} 0
            kubernetes_feature_enabled{name="AggregatedDiscoveryRemoveBetaType",stage="DEPRECATED"} 1
            kubernetes_feature_enabled{name="AllAlpha",stage="ALPHA"} 0
            kubernetes_feature_enabled{name="AllBeta",stage="BETA"} 0
            kubernetes_feature_enabled{name="AllowDNSOnlyNodeCSR",stage="DEPRECATED"} 0
        "#};

        assert!(FeatureGate::parse_from_metrics(response).is_ok());
    }

    #[rstest]
    #[case(r#"kubernetes_feature_disabled{name="AggregatedDiscoveryRemoveBetaType",stage="DEPRECATED"} 1"#)]
    #[case(r#"kubernetes_feature_enabled{name="APIResponseCompression",stage="GAMMA"} 1"#)]
    #[case(r#"kubernetes_feature_enabled{name="APIResponseCompression"} 1"#)]
    #[case(r#"kubernetes_feature_enabled{="APIResponseCompression",="ALPHA"} 1"#)]
    #[case("kubernetes_feature_enabled{} 0")]
    #[case("")]
    fn parse_feature_gate_invalid(#[case] input: &str) {
        assert!(FeatureGate::from_str(input).is_err());
    }
}
