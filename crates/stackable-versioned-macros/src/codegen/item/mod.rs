use darling::util::IdentString;
use syn::{Path, Type};

mod field;
pub use field::*;

mod variant;
pub use variant::*;

#[derive(Debug, PartialEq, Eq)]
pub enum ItemStatus {
    Addition {
        ident: IdentString,
        default_fn: Path,
        // NOTE (@Techassi): We need to carry idents and type information in
        // nearly every status. Ideally, we would store this in separate maps.
        ty: Box<Type>,
    },
    Change {
        downgrade_with: Option<Path>,
        upgrade_with: Option<Path>,
        from_ident: IdentString,
        to_ident: IdentString,
        from_type: Box<Type>,
        to_type: Box<Type>,
    },
    Deprecation {
        previous_ident: IdentString,
        note: Option<String>,
        ident: IdentString,
    },
    NoChange {
        previously_deprecated: bool,
        ident: IdentString,
        ty: Box<Type>,
    },
    NotPresent,
}

impl ItemStatus {
    pub fn get_ident(&self) -> &IdentString {
        match &self {
            Self::Addition { ident, .. }
            | Self::Change {
                to_ident: ident, ..
            }
            | Self::Deprecation { ident, .. }
            | Self::NoChange { ident, .. } => ident,
            Self::NotPresent => unreachable!("ItemStatus::NotPresent does not have an ident"),
        }
    }
}
