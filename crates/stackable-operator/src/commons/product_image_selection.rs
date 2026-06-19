use dockerfile_parser::ImageRef;
use k8s_openapi::api::core::v1::LocalObjectReference;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use stackable_shared::semver::{VersionExt, ZERO_ZERO_ZERO_DEV};

use crate::kvp::{LABEL_VALUE_MAX_LEN, LabelValue, LabelValueError};

pub const STACKABLE_DOCKER_REPO: &str = "oci.stackable.tech/sdp";

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("could not parse or create label from app version {app_version:?}"))]
    ParseAppVersionLabel {
        source: LabelValueError,
        app_version: String,
    },
}

/// Specify which image to use, the easiest way is to only configure the `productVersion`.
/// You can also configure a custom image registry to pull from, as well as completely custom
/// images.
///
/// Consult the [Product image selection documentation](DOCS_BASE_URL_PLACEHOLDER/concepts/product_image_selection)
/// for details.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductImage {
    #[serde(flatten)]
    image_selection: ProductImageSelection,

    #[serde(default)]
    /// [Pull policy](https://kubernetes.io/docs/concepts/containers/images/#image-pull-policy) used when pulling the image.
    pull_policy: Option<PullPolicy>,

    /// [Image pull secrets](https://kubernetes.io/docs/concepts/containers/images/#specifying-imagepullsecrets-on-a-pod) to pull images from a private registry.
    pull_secrets: Option<Vec<LocalObjectReference>>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum ProductImageSelection {
    // NOTE: Order matters!
    // The variants will be tried from top to bottom
    Custom(CustomProductImage),
    Auto(AutoProductImage),
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomProductImage {
    /// Provide a custom container image.
    ///
    /// Specify the full container image name, e.g. `oci.example.tech/namespace/superset:1.4.1-my-tag`
    custom: String,

    /// Version of the product, e.g. `1.4.1`.
    product_version: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoProductImage {
    /// Version of the product, e.g. `1.4.1`.
    product_version: String,

    /// Stackable version of the product, e.g. `26.7.0` or `0.0.0-dev`.
    ///
    /// If not specified, the operator will use its own version, e.g. `26.7.1`. When using a nightly
    /// operator or a PR version, it will use the nightly `0.0.0-dev` image.
    #[schemars(with = "Option::<String>")]
    stackable_version: Option<semver::Version>,

    /// Use a floating tag for the product image. Defaults to `false`.
    ///
    /// This mechanism utilizes a floating image tag which always refers to the latest patch version
    /// in the current release line. The current release line is either automatically derived by the
    /// operator based on its own version, or can be overridden with `stackableVersion`.
    ///
    /// A potential newer image is only pulled when Pods are rotated or their containers are
    /// restarted. Pods are NOT rotated and containers are NOT restarted automatically when a new
    /// image is available. This behaviour makes this a passive update mechanism, rather than an
    /// active one.
    ///
    /// It should be noted that when this field is set to `true`, the operator uses `Always` as the
    /// pull policy for product images. If set to `false`, `IfNotPresent` is used. Explicitly
    /// setting `pullPolicy` takes precedence.
    ///
    /// ### Examples
    ///
    /// - The `stackableVersion` field is not set, the operator falls back to its own version, eg.
    ///   26.7.0. If this field is set to `true`, the `26.7` floating tag will be used for product
    ///   images, else, `26.7.0` will be used.
    /// - The `stackableVersion` field is set to `26.3.0`. If this field is set to `true`, the
    ///   `26.3` floating tag will be used for product images, else, `26.3.0` will be used.
    #[serde(default)]
    use_floating_tag: bool,

    /// The repository on the container image registry where the container image is located, e.g.
    /// `oci.example.com/namespace`.
    ///
    /// If not specified, the operator will use the image registry provided via the operator
    /// environment options.
    repo: Option<String>,
}

#[derive(Clone, Debug, PartialEq, JsonSchema)]
pub struct ResolvedProductImage {
    /// Version of the product, e.g. `1.4.1`.
    pub product_version: String,

    /// App version formatted for Labels
    pub app_version_label_value: LabelValue,

    /// Image to be used for the product image e.g. `oci.stackable.tech/sdp/superset:1.4.1-stackable2.1.0`
    pub image: String,

    /// Image pull policy for the containers using the product image
    pub image_pull_policy: String,

    /// Image pull secrets for the containers using the product image
    pub pull_secrets: Option<Vec<LocalObjectReference>>,
}

/// TODO: Update comment
///
/// ### See
///
/// - <https://kubernetes.io/docs/concepts/containers/images/#image-pull-policy>
/// - <https://github.com/kubernetes/kubernetes/blob/master/pkg/apis/core/types.go#L2291-L2300>
#[derive(
    Copy,
    Clone,
    Debug,
    Deserialize,
    Eq,
    JsonSchema,
    PartialEq,
    Serialize,
    strum::AsRefStr,
    strum::Display,
)]
#[serde(rename = "PascalCase")]
pub enum PullPolicy {
    IfNotPresent,
    Always,
    Never,
}

impl PullPolicy {
    /// Returns the appropriate [`PullPolicy`] based on if a floating tag is used.
    fn from_is_floating_tag(is_floating_tag: bool) -> Self {
        match is_floating_tag {
            true => Self::Always,
            false => Self::IfNotPresent,
        }
    }
}

impl ProductImage {
    /// Resolves the product image to be used for containers.
    ///
    /// ### Parameters
    ///
    /// - `image_name`: The final part of the complete image reference, the name of the image.
    ///   Example values: `airflow` or `nginx`.
    /// - `image_repository`: The default repository consisting of a registry host and path. This
    ///   value should come from the operator environment options, which are provided via Helm for
    ///   example. Example value: `oci.example.org/my/namespace`.
    /// - `operator_version`: The version must be the full operator version and a valid semver
    ///   string. Accepted values are `23.7.0`, `0.0.0-dev` or `0.0.0-pr123`. Other variants are not
    ///   supported.
    ///
    /// ### Resolve mechanism
    ///
    /// The final product image is resolved in one of two ways defined by the [`ProductImageSelection`]:
    ///
    /// 1. When [`ProductImageSelection::Auto`] is selected by the user, the final product image
    ///    will be constructed based on the (user) provided values.
    /// 2. When [`ProductImageSelection::Custom`] is selected by the user, the final product image
    ///    will be the exact value specified by the user.
    //
    // FIXME (@Techassi): Make this take self instead of &self
    pub fn resolve(
        &self,
        image_name: &str,
        image_repository: &str,
        operator_version: &semver::Version,
    ) -> Result<ResolvedProductImage, Error> {
        let Self {
            image_selection,
            pull_policy,
            pull_secrets,
        } = self;

        // Keep track if a tag we consider floating is used. Currently, 0.0.0-dev, latest and YY.MM
        // (like 26.7) tags are considered floating.
        let mut is_floating_tag = false;

        match image_selection {
            ProductImageSelection::Custom(CustomProductImage {
                custom,
                product_version,
            }) => {
                let image_ref = ImageRef::parse(custom);
                let image_tag_or_hash = image_ref.tag.or(image_ref.hash).unwrap_or_else(|| {
                    is_floating_tag = true;
                    "latest".to_owned()
                });

                let app_version = format!("{product_version}-{image_tag_or_hash}");
                let app_version_label_value = Self::prepare_app_version_label_value(&app_version)?;
                let image_pull_policy = pull_policy
                    .unwrap_or_else(|| PullPolicy::from_is_floating_tag(is_floating_tag))
                    .to_string();

                Ok(ResolvedProductImage {
                    product_version: product_version.to_owned(),
                    pull_secrets: pull_secrets.clone(),
                    image: custom.to_owned(),
                    app_version_label_value,
                    image_pull_policy,
                })
            }
            ProductImageSelection::Auto(AutoProductImage {
                stackable_version,
                product_version,
                use_floating_tag,
                repo,
            }) => {
                let image_repository = repo
                    .as_deref()
                    .unwrap_or(image_repository)
                    // Remove and leading and trailing whitespace
                    .trim()
                    // Trim the end to ensure no double slashes are produced below
                    .trim_end_matches('/');

                let stackable_version = match stackable_version {
                    Some(version) => version,
                    None => {
                        if operator_version.major == 0
                            && operator_version.minor == 0
                            && operator_version.patch == 0
                            && operator_version.pre.starts_with("pr")
                        {
                            tracing::warn!(
                                %operator_version,
                                "operator is built by pull request, using {version} build of product image",
                                version = *ZERO_ZERO_ZERO_DEV
                            );

                            is_floating_tag = true;
                            &*ZERO_ZERO_ZERO_DEV
                        } else {
                            operator_version
                        }
                    }
                };

                let stackable_version = if *use_floating_tag {
                    is_floating_tag = true;
                    stackable_version.floating()
                } else {
                    stackable_version.to_string()
                };

                let image_pull_policy = match pull_policy {
                    Some(pull_policy) => {
                        if is_floating_tag && *pull_policy != PullPolicy::Always {
                            tracing::warn!(
                                pull_policy.configured = %pull_policy,
                                stackable_version,
                                r#"product image pull policy is not "Always" but a floating tag is \
                                used. This can lead to unexpected behaviour and it is recommended \
                                to explicitly set the pull policy to "Always" or let the operator \
                                derive it automatically by removing the pullPolicy field."#
                            );
                        }
                        pull_policy.to_string()
                    }
                    None => PullPolicy::from_is_floating_tag(is_floating_tag).to_string(),
                };

                // Trim leading ans trailing whitespace and also trim the start to ensure no double
                // slashes are produced below
                let image_name = image_name.trim().trim_start_matches('/');
                let app_version = format!("{product_version}-stackable{stackable_version}");
                let app_version_label_value = Self::prepare_app_version_label_value(&app_version)?;
                let image = format!("{image_repository}/{image_name}:{app_version}");

                Ok(ResolvedProductImage {
                    product_version: product_version.to_owned(),
                    pull_secrets: pull_secrets.clone(),
                    app_version_label_value,
                    image_pull_policy,
                    image,
                })
            }
        }
    }

    /// The product version is always known without having to resolve the image.
    /// In the future we might have a more clever version, which let's the operator pick a recommended product version
    /// automatically, e.g. from the LTS release line.
    pub fn product_version(&self) -> &str {
        match &self.image_selection {
            ProductImageSelection::Custom(CustomProductImage {
                product_version: pv,
                ..
            })
            | ProductImageSelection::Auto(AutoProductImage {
                product_version: pv,
                ..
            }) => pv,
        }
    }

    fn prepare_app_version_label_value(app_version: &str) -> Result<LabelValue, Error> {
        let mut formatted_app_version = app_version.to_string();
        // Labels cannot have more than `LABEL_VALUE_MAX_LEN` characters.
        formatted_app_version.truncate(LABEL_VALUE_MAX_LEN);
        // The hash has the format `sha256:85fa483aa99b9997ce476b86893ad5ed81fb7fd2db602977eb`
        // As the colon (`:`) is not a valid label value character, we replace it with a valid "-" character.
        let formatted_app_version = formatted_app_version.replace(':', "-");

        formatted_app_version
            .parse()
            .with_context(|_| ParseAppVersionLabelSnafu {
                app_version: formatted_app_version,
            })
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::auto_with_leading_slash_in_name(
        "/superset",
        "oci.stackable.tech/sdp",
        "23.7.42",
        r"
        productVersion: 1.4.1
        ",
        ResolvedProductImage {
            image: "oci.stackable.tech/sdp/superset:1.4.1-stackable23.7.42".to_string(),
            app_version_label_value: "1.4.1-stackable23.7.42".parse().expect("static app version label is always valid"),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::auto_without_stackable_version_stable_version(
        "superset",
        "oci.stackable.tech/sdp",
        "23.7.42",
        r"
        productVersion: 1.4.1
        ",
        ResolvedProductImage {
            image: "oci.stackable.tech/sdp/superset:1.4.1-stackable23.7.42".to_string(),
            app_version_label_value: "1.4.1-stackable23.7.42".parse().expect("static app version label is always valid"),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::auto_without_stackable_version_nightly(
        "superset",
        "oci.stackable.tech/sdp",
        "0.0.0-dev",
        r"
        productVersion: 1.4.1
        ",
        ResolvedProductImage {
            image: "oci.stackable.tech/sdp/superset:1.4.1-stackable0.0.0-dev".to_string(),
            app_version_label_value: "1.4.1-stackable0.0.0-dev".parse().expect("static app version label is always valid"),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::auto_without_stackable_version_pr_version(
        "superset",
        "oci.stackable.tech/sdp",
        "0.0.0-pr123",
        r"
        productVersion: 1.4.1
        ",
        ResolvedProductImage {
            image: "oci.stackable.tech/sdp/superset:1.4.1-stackable0.0.0-dev".to_string(),
            app_version_label_value: "1.4.1-stackable0.0.0-dev".parse().expect("static app version label is always valid"),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::auto_without_repo(
        "superset",
        "oci.stackable.tech/sdp",
        "23.7.42",
        r"
        productVersion: 1.4.1
        stackableVersion: 2.1.0
        ",
        ResolvedProductImage {
            image: "oci.stackable.tech/sdp/superset:1.4.1-stackable2.1.0".to_string(),
            app_version_label_value: "1.4.1-stackable2.1.0".parse().expect("static app version label is always valid"),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::auto_with_repository(
        "superset",
        "oci.stackable.tech/sdp",
        "23.7.42",
        r"
        productVersion: 1.4.1
        stackableVersion: 2.1.0
        repo: quay.io/stackable
        ",
        ResolvedProductImage {
            image: "quay.io/stackable/superset:1.4.1-stackable2.1.0".to_string(),
            app_version_label_value: "1.4.1-stackable2.1.0".parse().expect("static app version label is always valid"),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::auto_with_repository_trailing_slash(
        "superset",
        "oci.stackable.tech/sdp",
        "23.7.42",
        r"
        productVersion: 1.4.1
        stackableVersion: 2.1.0
        repo: quay.io/stackable/
        ",
        ResolvedProductImage {
            image: "quay.io/stackable/superset:1.4.1-stackable2.1.0".to_string(),
            app_version_label_value: "1.4.1-stackable2.1.0".parse().expect("static app version label is always valid"),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::auto_with_use_floating_tag(
        "superset",
        "oci.stackable.tech/sdp",
        "23.7.42",
        r"
        productVersion: 1.4.1
        useFloatingTag: true
        ",
        ResolvedProductImage {
            image: "oci.stackable.tech/sdp/superset:1.4.1-stackable23.7".to_owned(),
            app_version_label_value: "1.4.1-stackable23.7".parse().expect("static app version label is always valid"),
            product_version: "1.4.1".to_owned(),
            image_pull_policy: "Always".to_owned(),
            pull_secrets: None
        }
    )]
    #[case::auto_with_use_floating_tag_and_stackable_version(
        "superset",
        "oci.stackable.tech/sdp",
        "23.7.42",
        r"
        productVersion: 1.4.1
        stackableVersion: 2.1.0
        useFloatingTag: true
        ",
        ResolvedProductImage {
            image: "oci.stackable.tech/sdp/superset:1.4.1-stackable2.1".to_owned(),
            app_version_label_value: "1.4.1-stackable2.1".parse().expect("static app version label is always valid"),
            product_version: "1.4.1".to_owned(),
            image_pull_policy: "Always".to_owned(),
            pull_secrets: None
        }
    )]
    #[case::auto_with_use_floating_tag_and_pull_policy(
        "superset",
        "oci.stackable.tech/sdp",
        "23.7.42",
        r"
        productVersion: 1.4.1
        pullPolicy: IfNotPresent
        useFloatingTag: true
        ",
        ResolvedProductImage {
            image: "oci.stackable.tech/sdp/superset:1.4.1-stackable23.7".to_owned(),
            app_version_label_value: "1.4.1-stackable23.7".parse().expect("static app version label is always valid"),
            product_version: "1.4.1".to_owned(),
            image_pull_policy: "IfNotPresent".to_owned(),
            pull_secrets: None
        }
    )]
    #[case::custom_without_tag(
        "superset",
        "oci.stackable.tech/sdp",
        "23.7.42",
        r"
        custom: my.corp/myteam/stackable/superset
        productVersion: 1.4.1
        ",
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset".to_string(),
            app_version_label_value: "1.4.1-latest".parse().expect("static app version label is always valid"),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::custom_with_tag(
        "superset",
        "oci.stackable.tech/sdp",
        "23.7.42",
        r"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        ",
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset:latest-and-greatest".to_string(),
            app_version_label_value: "1.4.1-latest-and-greatest".parse().expect("static app version label is always valid"),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::custom_with_colon_in_repo_and_without_tag(
        "superset",
        "oci.stackable.tech/sdp",
        "23.7.42",
        r"
        custom: 127.0.0.1:8080/myteam/stackable/superset
        productVersion: 1.4.1
        ",
        ResolvedProductImage {
            image: "127.0.0.1:8080/myteam/stackable/superset".to_string(),
            app_version_label_value: "1.4.1-latest".parse().expect("static app version label is always valid"),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::custom_with_colon_in_repo_and_with_tag(
        "superset",
        "oci.stackable.tech/sdp",
        "23.7.42",
        r"
        custom: 127.0.0.1:8080/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        ",
        ResolvedProductImage {
            image: "127.0.0.1:8080/myteam/stackable/superset:latest-and-greatest".to_string(),
            app_version_label_value: "1.4.1-latest-and-greatest".parse().expect("static app version label is always valid"),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::custom_with_hash_in_repo_and_without_tag(
        "superset",
        "oci.stackable.tech/sdp",
        "23.7.42",
        r"
        custom: oci.stackable.tech/sdp/superset@sha256:85fa483aa99b9997ce476b86893ad5ed81fb7fd2db602977eb8c42f76efc1098
        productVersion: 1.4.1
        ",
        ResolvedProductImage {
            image: "oci.stackable.tech/sdp/superset@sha256:85fa483aa99b9997ce476b86893ad5ed81fb7fd2db602977eb8c42f76efc1098".to_string(),
            app_version_label_value: "1.4.1-sha256-85fa483aa99b9997ce476b86893ad5ed81fb7fd2db602977eb".parse().expect("static app version label is always valid"),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::custom_takes_precedence(
        "superset",
        "oci.stackable.tech/sdp",
        "23.7.42",
        r"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        stackableVersion: not-used
        ",
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset:latest-and-greatest".to_string(),
            app_version_label_value: "1.4.1-latest-and-greatest".parse().expect("static app version label is always valid"),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::pull_policy_if_not_present(
        "superset",
        "oci.stackable.tech/sdp",
        "23.7.42",
        r"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        pullPolicy: IfNotPresent
        ",
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset:latest-and-greatest".to_string(),
            app_version_label_value: "1.4.1-latest-and-greatest".parse().expect("static app version label is always valid"),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::pull_policy_always(
        "superset",
        "oci.stackable.tech/sdp",
        "23.7.42",
        r"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        pullPolicy: Always
        ",
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset:latest-and-greatest".to_string(),
            app_version_label_value: "1.4.1-latest-and-greatest".parse().expect("static app version label is always valid"),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::pull_policy_never(
        "superset",
        "oci.stackable.tech/sdp",
        "23.7.42",
        r"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        pullPolicy: Never
        ",
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset:latest-and-greatest".to_string(),
            app_version_label_value: "1.4.1-latest-and-greatest".parse().expect("static app version label is always valid"),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Never".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::pull_secrets(
        "superset",
        "oci.stackable.tech/sdp",
        "23.7.42",
        r"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        pullPolicy: Always
        pullSecrets:
        - name: myPullSecrets1
        - name: myPullSecrets2
        ",
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset:latest-and-greatest".to_string(),
            app_version_label_value: "1.4.1-latest-and-greatest".parse().expect("static app version label is always valid"),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: Some(vec![LocalObjectReference{name: "myPullSecrets1".to_string()}, LocalObjectReference{name: "myPullSecrets2".to_string()}]),
        }
    )]
    fn resolved_image_pass(
        #[case] image_name: String,
        #[case] image_repository: String,
        #[case] operator_version: String,
        #[case] input: String,
        #[case] expected: ResolvedProductImage,
    ) {
        let product_image: ProductImage = serde_yaml::from_str(&input).expect("Illegal test input");
        let operator_version = operator_version.parse().expect("invalid operator version");
        let resolved_product_image = product_image
            .resolve(&image_name, &image_repository, &operator_version)
            .expect("Illegal test input");

        assert_eq!(resolved_product_image, expected);
    }

    #[rstest]
    #[case::custom(
        r"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        ",
        "data did not match any variant of untagged enum ProductImageSelection at line 2 column 9"
    )]
    #[case::stackable_version(
        r"
        stackableVersion: 2.1.0
        ",
        "data did not match any variant of untagged enum ProductImageSelection at line 2 column 9"
    )]
    #[case::empty(
        "{}",
        "data did not match any variant of untagged enum ProductImageSelection"
    )]
    fn resolved_image_fail(#[case] input: String, #[case] expected: String) {
        let err = serde_yaml::from_str::<ProductImage>(&input).expect_err("Must be error");

        assert_eq!(err.to_string(), expected);
    }
}
