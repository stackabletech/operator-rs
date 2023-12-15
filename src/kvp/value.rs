use std::{
    error::Error,
    fmt::{Debug, Display},
    ops::Deref,
    str::FromStr,
};

/// Trait which ensures the value of [`KeyValuePair`][crate::kvp::KeyValuePair]
/// is validated. Different value implementations should use [`FromStr`] to
/// parse and validate the value based on the requirements.
pub trait Value:
    Deref<Target = str> + FromStr<Err = Self::Error> + Clone + Display + Eq + Ord
{
    type Error: Error + Debug + 'static;
}
