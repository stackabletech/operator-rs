use std::{
    error::Error,
    fmt::{Debug, Display},
    ops::Deref,
    str::FromStr,
};

use serde::Serialize;

/// Trait which ensures the value of [`KeyValuePair`][crate::kvp::KeyValuePair]
/// is validated. Different value implementations should use [`FromStr`] to
/// parse and validate the value based on the requirements.
pub trait ValueExt:
    Deref<Target = String> + FromStr<Err = Self::Error> + Clone + Display + Eq + Ord + Serialize
{
    type Error: Error + Debug + 'static;
}