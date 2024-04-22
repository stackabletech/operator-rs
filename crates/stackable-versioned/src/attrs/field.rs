use darling::{util::SpannedValue, Error, FromField, FromMeta};
use k8s_version::Version;
use syn::{Field, Ident};

use crate::gen::version::ContainerVersion;

#[derive(Debug, FromField)]
#[darling(attributes(versioned), forward_attrs(allow, doc, cfg, serde))]
pub(crate) struct FieldAttributes {
    ident: Option<Ident>,
    added: Option<AddedAttributes>,

    #[darling(multiple)]
    renamed: Vec<RenamedAttributes>,

    deprecated: Option<DeprecatedAttributes>,
}

#[derive(Debug, FromMeta)]
pub(crate) struct AddedAttributes {
    pub(crate) since: SpannedValue<Version>,
}

#[derive(Debug, FromMeta)]
pub(crate) struct RenamedAttributes {
    pub(crate) since: SpannedValue<Version>,
    pub(crate) _from: SpannedValue<String>,
}

#[derive(Debug, FromMeta)]
pub(crate) struct DeprecatedAttributes {
    pub(crate) since: SpannedValue<Version>,
    pub(crate) _note: SpannedValue<String>,
}

/// This struct describes all possible actions which can be attached to _one_
/// field.
///
/// - A field can only ever be added once at most. A field not marked as 'added'
///   is part of the struct in every version until renamed or deprecated.
/// - A field can be renamed many times. That's why renames are stored in a
///   [`Vec`].
/// - A field can only be deprecated once. A field not marked as 'deprecated'
///   will be included up until the latest version.
#[derive(Debug)]
pub(crate) struct FieldActions {
    added: Option<AddedAttributes>,
    renamed: Vec<RenamedAttributes>,
    deprecated: Option<DeprecatedAttributes>,
}

impl TryFrom<FieldAttributes> for FieldActions {
    type Error = Error;

    fn try_from(attrs: FieldAttributes) -> Result<Self, Self::Error> {
        match (&attrs.added, &attrs.renamed, &attrs.deprecated) {
            (Some(added), _, Some(deprecated)) => {
                if *added.since == *deprecated.since {
                    return Err(Error::custom(
                        "field cannot be marked as `added` and `deprecated` in the same version",
                    )
                    .with_span(&attrs.ident.expect("internal: field must have name").span()));
                }
            }
            (Some(added), renamed, _) => {
                if renamed.iter().any(|r| *r.since == *added.since) {
                    return Err(Error::custom(
                        "field cannot be marked as `added` and `renamed` in the same version",
                    )
                    .with_span(&attrs.ident.expect("internal: field must have name").span()));
                }
            }
            (_, renamed, Some(deprecated)) => {
                if renamed.iter().any(|r| *r.since == *deprecated.since) {
                    return Err(Error::custom(
                        "field cannot be marked as `deprecated` and `renamed` in the same version",
                    )
                    .with_span(&attrs.ident.expect("internal: field must have name").span()));
                }
            }
            _ => {}
        }

        Ok(Self {
            added: attrs.added,
            renamed: attrs.renamed,
            deprecated: attrs.deprecated,
        })
    }
}

impl FieldActions {
    pub(crate) fn is_in_version_set(
        &self,
        versions: &[ContainerVersion],
        field: &Field,
    ) -> Result<(), Error> {
        // NOTE (@Techassi): Can we maybe optimize this a little?

        if let Some(added) = &self.added {
            if !versions.iter().any(|v| v.inner == *added.since) {
                return Err(
                    Error::custom("field action `added` uses version which was not declared via #[versioned(version)]")
                    .with_span(&field.ident.as_ref().expect("internal: field must have name").span()
                ));
            }
        }

        for rename in &self.renamed {
            if !versions.iter().any(|v| v.inner == *rename.since) {
                return Err(Error::custom("field action `renamed` uses version which was not declared via #[versioned(version)]"));
            }
        }

        if let Some(deprecated) = &self.deprecated {
            if !versions.iter().any(|v| v.inner == *deprecated.since) {
                return Err(Error::custom("field action `deprecated` uses version which was not declared via #[versioned(version)]"));
            }
        }

        Ok(())
    }
}
