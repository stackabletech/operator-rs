use k8s_openapi::api::core::v1::LocalObjectReference;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::AsRefStr;

pub const STACKABLE_DOCKER_REPO: &str = "docker.stackable.tech/stackable";

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductImage {
    #[serde(flatten)]
    image_selection: ProductImageSelection,

    #[serde(default)]
    /// [Pull policy](https://kubernetes.io/docs/concepts/containers/images/#image-pull-policy) used when pulling the Images
    pull_policy: PullPolicy,

    /// [Image pull secrets](https://kubernetes.io/docs/concepts/containers/images/#specifying-imagepullsecrets-on-a-pod) to pull images from a private registry
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
    // The following enum variant is commented out for now, as the operators currently don't know which stackableVersions they are compatible with.
    // In the future they should be able to automatically pick a fitting stackableVersion for them.
    // The concept of the untagged enum was tested with all known upcoming variants to make sure we don't run into strange problems in the future.
    // They code snippets are left for illustration and can be commented in as soon as the operators can pick the stackableVersions automatically.
    Stackable(ProductImageStackable),
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductImageCustom {
    /// Overwrite the docker image.
    /// Specify the full docker image name, e.g. `docker.stackable.tech/stackable/superset:1.4.1-stackable2.1.0`
    custom: String,
    /// Version of the product, e.g. `1.4.1`.
    product_version: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductImageStackableVersion {
    /// Version of the product, e.g. `1.4.1`.
    product_version: String,
    /// Stackable version of the product, e.g. 2.1.0
    stackable_version: String,
    /// Name of the docker repo, e.g. `docker.stackable.tech/stackable`
    repo: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductImageStackable {
    /// Version of the product, e.g. `1.4.1`.
    /// Set it to `auto` to let the operator automatically pick the recommended product version he is compatible with.
    // Note that this is not an Option<String>, as in this case no attribute is needed for this enum variant and this enum variant will match *any* arbitrary input,
    // thus making the validation useless
    product_version: String,
    /// Name of the docker repo, e.g. `docker.stackable.tech/stackable`
    repo: Option<String>,
}

#[derive(Clone, Debug, PartialEq, JsonSchema)]
pub struct ResolvedProductImage {
    pub product_version: String,
    pub image: String,
    pub image_pull_policy: String,
    pub pull_secrets: Option<Vec<LocalObjectReference>>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename = "PascalCase")]
#[derive(AsRefStr)]
pub enum PullPolicy {
    IfNotPresent,
    Always,
    Never,
}

impl Default for PullPolicy {
    fn default() -> PullPolicy {
        PullPolicy::IfNotPresent
    }
}

impl ProductImage {
    pub fn resolve(&self, image_base_name: &str) -> ResolvedProductImage {
        let image_pull_policy = self.pull_policy.as_ref().to_string();
        let pull_secrets = self.pull_secrets.clone();

        match &self.image_selection {
            ProductImageSelection::Custom(custom) => ResolvedProductImage {
                product_version: custom.product_version.to_string(),
                image: custom.custom.to_string(),
                image_pull_policy,
                pull_secrets,
            },
            ProductImageSelection::StackableVersion(stackable_version) => {
                let repo = stackable_version
                    .repo
                    .as_deref()
                    .unwrap_or(STACKABLE_DOCKER_REPO);
                let image = format!(
                    "{repo}/{image_base_name}:{product_version}-stackable{stackable_version}",
                    product_version = stackable_version.product_version,
                    stackable_version = stackable_version.stackable_version,
                );
                ResolvedProductImage {
                    product_version: stackable_version.product_version.to_string(),
                    image,
                    image_pull_policy,
                    pull_secrets,
                }
            }
            ProductImageSelection::Stackable(stackable) => {
                let product_version = match stackable.product_version.as_str() {
                    "auto" => "TODO".to_string(),
                    product_version => product_version.to_string(),
                };
                let stackable_version = "TODO";
                let repo = stackable.repo.as_deref().unwrap_or(STACKABLE_DOCKER_REPO);

                let image = format!(
                    "{repo}/{image_base_name}:{product_version}-stackable{stackable_version}"
                );
                ResolvedProductImage {
                    product_version,
                    image,
                    image_pull_policy,
                    pull_secrets,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rstest::rstest;

    #[rstest]
    #[case::stackable_version_without_repo(
        "superset",
        r#"
        productVersion: 1.4.1
        stackableVersion: 2.1.0
        "#,
        ResolvedProductImage {
            image: "docker.stackable.tech/stackable/superset:1.4.1-stackable2.1.0".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::stackable_version_with_repo(
        "trino",
        r#"
        productVersion: 1.4.1
        stackableVersion: 2.1.0
        repo: my.corp/myteam/stackable
        "#,
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/trino:1.4.1-stackable2.1.0".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::custom(
        "superset",
        r#"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        "#,
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset:latest-and-greatest".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::stackable_without_repo(
        "superset",
        r#"
        productVersion: 1.4.1
        "#,
        ResolvedProductImage {
            image: "docker.stackable.tech/stackable/superset:1.4.1-stackableTODO".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::default(
        "superset",
        r#"
        productVersion: auto
        "#,
        ResolvedProductImage {
            image: "docker.stackable.tech/stackable/superset:TODO-stackableTODO".to_string(),
            product_version: "TODO".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::stackable_with_repo(
        "superset",
        r#"
        productVersion: 1.4.1
        repo: my.corp/myteam/stackable
        "#,
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset:1.4.1-stackableTODO".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::custom_takes_precedence(
        "superset",
        r#"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        stackableVersion: not-used
        "#,
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset:latest-and-greatest".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::pull_policy_if_not_present(
        "superset",
        r#"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        repo: not-used
        pullPolicy: IfNotPresent
        "#,
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset:latest-and-greatest".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::pull_policy_always(
        "superset",
        r#"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        repo: not-used
        pullPolicy: Always
        "#,
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset:latest-and-greatest".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::pull_policy_never(
        "superset",
        r#"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        repo: not-used
        pullPolicy: Never
        "#,
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset:latest-and-greatest".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Never".to_string(),
            pull_secrets: None,
        }
    )]
    #[case::pull_secrets(
        "superset",
        r#"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        repo: not-used
        pullPolicy: Always
        pullSecrets:
        - name: myPullSecrets1
        - name: myPullSecrets2
        "#,
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset:latest-and-greatest".to_string(),
            product_version: "1.4.1".to_string(),
            image_pull_policy: "Always".to_string(),
            pull_secrets: Some(vec![LocalObjectReference{name: Some("myPullSecrets1".to_string())}, LocalObjectReference{name: Some("myPullSecrets2".to_string())}]),
        }
    )]
    fn test_correct_resolved_image(
        #[case] image_base_name: String,
        #[case] input: String,
        #[case] expected: ResolvedProductImage,
    ) {
        let product_image: ProductImage = serde_yaml::from_str(&input).expect("Illegal test input");
        let resolved_product_image = product_image.resolve(&image_base_name);

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
    fn test_invalid_image(#[case] input: String, #[case] expected: String) {
        let err = serde_yaml::from_str::<ProductImage>(&input).expect_err("Must be error");

        assert_eq!(err.to_string(), expected);
    }
}
