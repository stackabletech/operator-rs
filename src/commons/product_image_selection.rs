use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const STACKABLE_DOCKER_REPO: &str = "docker.stackable.tech/stackable";

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
    custom: String,
    product_version: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductImageStackableVersion {
    product_version: String,
    stackable_version: String,
    repo: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductImageStackable {
    product_version: String,
    repo: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedProductImage {
    pub image: String,
    pub product_version: String,
}

impl ProductImageSelection {
    pub fn resolve(self, image_base_name: &str) -> ResolvedProductImage {
        match self {
            ProductImageSelection::Custom(custom) => ResolvedProductImage {
                image: custom.custom,
                product_version: custom.product_version,
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
                    image,
                    product_version: stackable_version.product_version,
                }
            }
            ProductImageSelection::Stackable(stackable) => {
                let stackable_version = "TODO";
                let repo = stackable.repo.as_deref().unwrap_or(STACKABLE_DOCKER_REPO);

                let image = format!(
                    "{repo}/{image_base_name}:{product_version}-stackable{stackable_version}",
                    product_version = stackable.product_version,
                );
                ResolvedProductImage {
                    image,
                    product_version: stackable.product_version,
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
        }
    )]
    #[case::custom_takes_precedence(
        "superset",
        r#"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        productVersion: 1.4.1
        repo: not-used
        "#,
        ResolvedProductImage {
            image: "my.corp/myteam/stackable/superset:latest-and-greatest".to_string(),
            product_version: "1.4.1".to_string(),
        }
    )]
    fn test_correct_resolved_image(
        #[case] product_image_base_name: String,
        #[case] input: String,
        #[case] expected: ResolvedProductImage,
    ) {
        let product_image:ProductImageSelection =
            serde_yaml::from_str(&input).expect("Illegal test input");
        let product_image = product_image.resolve(&product_image_base_name);

        assert_eq!(product_image, expected);
    }

    #[rstest]
    #[case::empty(
        "{}",
        "data did not match any variant of untagged enum ProductImageSelection"
    )]
    #[case::custom(
        r#"
        custom: my.corp/myteam/stackable/superset:latest-and-greatest
        "#,
        "data did not match any variant of untagged enum ProductImageSelection"
    )]
    #[case::stackable_version(
        r#"
        stackableVersion: 2.1.0
        "#,
        "data did not match any variant of untagged enum ProductImageSelection"
    )]
    fn test_invalid_image(
        #[case] input: String,
        #[case] expected: String,
    ) {
        let err = serde_yaml::from_str::<ProductImageSelection>(&input).expect_err("Must be error");

        assert_eq!(err.to_string(), expected);
    }
}
