use std::fmt;

use darling::{util::SpannedValue, Error, FromField, FromMeta};

#[derive(Debug, FromField)]
#[darling(attributes(versioned), forward_attrs(allow, doc, cfg, serde))]
pub(crate) struct FieldAttributes {
    added: Option<AddedAttributes>,
    renamed: Option<RenamedAttributes>,
    deprecated: Option<DeprecatedAttributes>,
}

#[derive(Debug, FromMeta)]
pub(crate) struct AddedAttributes {
    since: SpannedValue<String>,
}

#[derive(Debug, FromMeta)]
pub(crate) struct RenamedAttributes {
    since: SpannedValue<String>,
    _to: SpannedValue<String>,
}

#[derive(Debug, FromMeta)]
pub(crate) struct DeprecatedAttributes {
    since: SpannedValue<String>,
}

#[derive(Debug)]
pub(crate) enum FieldAction {
    Added(AddedAttributes),
    Renamed(RenamedAttributes),
    Deprecated(DeprecatedAttributes),
    None,
}

impl TryFrom<FieldAttributes> for FieldAction {
    type Error = Error;

    fn try_from(value: FieldAttributes) -> Result<Self, Self::Error> {
        // NOTE (@Techassi): We sadly currently cannot use the attribute span
        // when reporting errors. That's why the errors will be displayed at
        // the #[derive(Versioned)] position.

        match (value.added, value.renamed, value.deprecated) {
            (Some(added), None, None) => Ok(FieldAction::Added(added)),
            (None, Some(renamed), None) => Ok(FieldAction::Renamed(renamed)),
            (None, None, Some(deprecated)) => Ok(FieldAction::Deprecated(deprecated)),
            (None, None, None) => Ok(FieldAction::None),
            _ => Err(Error::custom(
                "cannot specifiy multiple field actions at once",
            )),
        }
    }
}

impl fmt::Display for FieldAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldAction::Added(_) => "added".fmt(f),
            FieldAction::Renamed(_) => "renamed".fmt(f),
            FieldAction::Deprecated(_) => "deprecated".fmt(f),
            FieldAction::None => "".fmt(f),
        }
    }
}

impl FieldAction {
    pub(crate) fn since(&self) -> Option<&str> {
        match self {
            FieldAction::Added(added) => Some(&*added.since),
            FieldAction::Renamed(renamed) => Some(&*renamed.since),
            FieldAction::Deprecated(deprecated) => Some(&*deprecated.since),
            FieldAction::None => None,
        }
    }
}
