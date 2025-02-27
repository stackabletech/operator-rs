use dockerfile_parser::ImageRef;
use k8s_openapi::api::core::v1::LocalObjectReference;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::AsRefStr;

#[cfg(doc)]
use crate::kvp::Labels;

pub const STACKABLE_DOCKER_REPO: &str = "oci.stackable.tech/sdp";

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
    pull_policy: PullPolicy,

    /// [Image pull secrets](https://kubernetes.io/docs/concepts/containers/images/#specifying-imagepullsecrets-on-a-pod) to pull images from a private registry.
    pull_secrets: Option<Vec<LocalObjectReference>>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum ProductImageSelection {
    // Order matters!
    // The variants will be tried from top to bottom
    Custom(ProductImageCustom),
    StackableVersion(ProductImageStackableVersion),
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductImageCustom {
    /// Overwrite the docker image.
    /// Specify the full docker image name, e.g. `oci.stackable.tech/sdp/superset:1.4.1-stackable2.1.0`
    custom: String,
    /// Version of the product, e.g. `1.4.1`.
    product_version: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductImageStackableVersion {
    /// Version of the product, e.g. `1.4.1`.
    product_version: String,
    /// Stackable version of the product, e.g. `23.4`, `23.4.1` or `0.0.0-dev`.
    /// If not specified, the operator will use its own version, e.g. `23.4.1`.
    /// When using a nightly operator or a pr version, it will use the nightly `0.0.0-dev` image.
    stackable_version: Option<String>,
    /// Name of the docker repo, e.g. `oci.stackable.tech/sdp`
    repo: Option<String>,
}

#[derive(Clone, Debug, PartialEq, JsonSchema)]
pub struct ResolvedProductImage {
    /// Version of the product, e.g. `1.4.1`.
    pub product_version: String,

    /// App version as formatted for [`Labels::recommended`]
    pub app_version_label: String,

    /// Image to be used for the product image e.g. `oci.stackable.tech/sdp/superset:1.4.1-stackable2.1.0`
    pub image: String,

    /// Image pull policy for the containers using the product image
    pub image_pull_policy: String,

    /// Image pull secrets for the containers using the product image
    pub pull_secrets: Option<Vec<LocalObjectReference>>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename = "PascalCase")]
#[derive(AsRefStr)]
/// We default to `Always`, as we use floating tags for our release lines.
/// This means the tag 23.4 starts of pointing to the same image 23.4.0 does, but switches to 23.4.1 after the releases of 23.4.1.
///
/// ### See
///
/// - <https://kubernetes.io/docs/concepts/containers/images/#image-pull-policy>
/// - <https://github.com/kubernetes/kubernetes/blob/master/pkg/apis/core/types.go#L2291-L2300>
pub enum PullPolicy {
    IfNotPresent,
    #[default]
    Always,
    Never,
}

impl ProductImage {
    /// `image_base_name` should be base of the image name in the container image registry, e.g. `trino`.
    /// `operator_version` needs to be the full operator version and a valid semver string.
    /// Accepted values are `23.7.0`, `0.0.0-dev` or `0.0.0-pr123`. Other variants are not supported.
    pub fn resolve(&self, image_base_name: &str, operator_version: &str) -> ResolvedProductImage {
        let image_pull_policy = self.pull_policy.as_ref().to_string();
        let pull_secrets = self.pull_secrets.clone();

        let product_version = self.product_version().to_owned();

        match &self.image_selection {
            ProductImageSelection::Custom(image_selection) => {
                let image = ImageRef::parse(&image_selection.custom);
                let image_tag_or_hash = image.tag.or(image.hash).unwrap_or("latest".to_string());
                let mut app_version_label = format!("{}-{}", product_version, image_tag_or_hash);
                // TODO Use new label mechanism once added
                app_version_label.truncate(63);

                ResolvedProductImage {
                    product_version,
                    app_version_label,
                    image: image_selection.custom.clone(),
                    image_pull_policy,
                    pull_secrets,
                }
            }
            ProductImageSelection::StackableVersion(image_selection) => {
                let repo = image_selection
                    .repo
                    .as_deref()
                    .unwrap_or(STACKABLE_DOCKER_REPO);
                let stackable_version = match &image_selection.stackable_version {
                    Some(stackable_version) => stackable_version,
                    None => {
                        if operator_version.starts_with("0.0.0-pr") {
                            let override_version = "0.0.0-dev";
                            tracing::warn!(
                                operator_version,
                                override_version,
                                "operator is built by pull request, using dev build of product image"
                            );
                            override_version
                        } else {
                            operator_version
                        }
                    }
                };
                let image = format!(
                    "{repo}/{image_base_name}:{product_version}-stackable{stackable_version}",
                );
                let app_version_label = format!("{product_version}-stackable{stackable_version}",);
                ResolvedProductImage {
                    product_version,
                    app_version_label,
                    image,
                    image_pull_policy,
                    pull_secrets,
                }
            }
        }
    }

    /// The product version is always known without having to resolve the image.
    /// In the future we might have a more clever version, which let's the operator pick a recommended product version
    /// automatically, e.g. from the LTS release line.
    pub fn product_version(&self) -> &str {
        match &self.image_selection {
            ProductImageSelection::Custom(ProductImageCustom {
                product_version: pv,
                ..
            }) => pv,
            ProductImageSelection::StackableVersion(ProductImageStackableVersion {
                product_version: pv,
                ..
            }) => pv,
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::stackable_version_without_stackable_version_stable_version(
        "superset",
        "23.7.42",
        r#"
        productVersion: 1.4.1
        "#,
        ResolvedProductImage {
            image: "oci.stackable.tech/sdp/superset:1.4.1-stackable23.7.42".to_string(),
            app_version_label: "1.4.1-stackable23.7.42".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::stackable_version_without_stackable_version_nightly(
        "superset",
        "0.0.0-dev",
        r#"
        productVersion: 1.4.1
        "#,
        ResolvedProductImage {
            image: "oci.stackable.tech/sdp/superset:1.4.1-stackable0.0.0-dev".to_string(),
            app_version_label: "1.4.1-stackable0.0.0-dev".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::stackable_version_without_stackable_version_pr_version(
        "superset",
        "0.0.0-pr123",
        r#"
        productVersion: 1.4.1
        "#,
        ResolvedProductImage {
            image: "oci.stackable.tech/sdp/superset:1.4.1-stackable0.0.0-dev".to_string(),
            app_version_label: "1.4.1-stackable0.0.0-dev".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::stackable_version_without_repo(
        "superset",
        "23.7.42",
        r#"
        productVersion: 1.4.1
        stackableVersion: 2.1.0
        "#,
        ResolvedProductImage {
            image: "oci.stackable.tech/sdp/superset:1.4.1-stackable2.1.0".to_string(),
            app_version_label: "1.4.1-stackable2.1.0".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::stackable_version_with_repo(
        "trino",
        "23.7.42",
        r#"
        productVersion: 1.4.1
        stackableVersion: 2.1.0
        repo: my.corp/myteam/stackable
        "#,
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/trino:1.4.1-stackable2.1.0".to_string(),
            app_version_label: "1.4.1-stackable2.1.0".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::custom_without_tag(
        "superset",
        "23.7.42",
        r#"
        custom: my.corp/myteam/stackable/superset
        productVersion: 1.4.1
        "#,
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset".to_string(),
            app_version_label: "1.4.1-latest".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::custom_with_tag(
        "superset",
        "23.7.42",
        r#"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        "#,
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset:latest-and-greatest".to_string(),
            app_version_label: "1.4.1-latest-and-greatest".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::custom_with_colon_in_repo_and_without_tag(
        "superset",
        "23.7.42",
        r#"
        custom: 127.0.0.1:8080/myteam/stackable/superset
        productVersion: 1.4.1
        "#,
        ResolvedProductImage {
            image: "127.0.0.1:8080/myteam/stackable/superset".to_string(),
            app_version_label: "1.4.1-latest".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::custom_with_colon_in_repo_and_with_tag(
        "superset",
        "23.7.42",
        r#"
        custom: 127.0.0.1:8080/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        "#,
        ResolvedProductImage {
            image: "127.0.0.1:8080/myteam/stackable/superset:latest-and-greatest".to_string(),
            app_version_label: "1.4.1-latest-and-greatest".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::custom_with_hash_in_repo_and_without_tag(
        "superset",
        "23.7.42",
        r#"
        custom: oci.stackable.tech/sdp/superset@sha256:85fa483aa99b9997ce476b86893ad5ed81fb7fd2db602977eb8c42f76efc1098
        productVersion: 1.4.1
        "#,
        ResolvedProductImage {
            image: "oci.stackable.tech/sdp/superset@sha256:85fa483aa99b9997ce476b86893ad5ed81fb7fd2db602977eb8c42f76efc1098".to_string(),
            app_version_label: "1.4.1-sha256:85fa483aa99b9997ce476b86893ad5ed81fb7fd2db602977eb".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::custom_takes_precedence(
        "superset",
        "23.7.42",
        r#"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        stackableVersion: not-used
        "#,
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset:latest-and-greatest".to_string(),
            app_version_label: "1.4.1-latest-and-greatest".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::pull_policy_if_not_present(
        "superset",
        "23.7.42",
        r#"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        pullPolicy: IfNotPresent
        "#,
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset:latest-and-greatest".to_string(),
            app_version_label: "1.4.1-latest-and-greatest".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::pull_policy_always(
        "superset",
        "23.7.42",
        r#"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        pullPolicy: Always
        "#,
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset:latest-and-greatest".to_string(),
            app_version_label: "1.4.1-latest-and-greatest".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::pull_policy_never(
        "superset",
        "23.7.42",
        r#"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        pullPolicy: Never
        "#,
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset:latest-and-greatest".to_string(),
            app_version_label: "1.4.1-latest-and-greatest".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Never".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::pull_secrets(
        "superset",
        "23.7.42",
        r#"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        pullPolicy: Always
        pullSecrets:
        - name: myPullSecrets1
        - name: myPullSecrets2
        "#,
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset:latest-and-greatest".to_string(),
            app_version_label: "1.4.1-latest-and-greatest".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: Some(vec![LocalObjectReference{name: "myPullSecrets1".to_string()}, LocalObjectReference{name: "myPullSecrets2".to_string()}]),
        }
    )]
    fn resolved_image_pass(
        #[case] image_base_name: String,
        #[case] operator_version: String,
        #[case] input: String,
        #[case] expected: ResolvedProductImage,
    ) {
        let product_image: ProductImage = serde_yaml::from_str(&input).expect("Illegal test input");
        let resolved_product_image = product_image.resolve(&image_base_name, &operator_version);

        assert_eq!(resolved_product_image, expected);
    }

    #[rstest]
    #[case::custom(
        r#"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        "#,
        "data did not match any variant of untagged enum ProductImageSelection at line 2 column 9"
    )]
    #[case::stackable_version(
        r#"
        stackableVersion: 2.1.0
        "#,
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
