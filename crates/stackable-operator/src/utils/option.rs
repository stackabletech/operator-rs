use std::{borrow::Cow, ops::Deref};

#[cfg(doc)]
use std::path::PathBuf;

/// Extension methods for [`Option`].
pub trait OptionExt<T> {
    /// Returns a reference to the value if [`Some`], otherwise evaluates `default()`.
    ///
    /// Compared to [`Option::unwrap_or_else`], this saves having to [`Clone::clone`] the value to make the types line up.
    ///
    /// Consider using [`Self::as_deref_or_else`] instead if the type implements [`Deref`] (such as [`String`] or [`PathBuf`]).
    fn as_ref_or_else(&self, default: impl FnOnce() -> T) -> Cow<T>
    where
        T: Clone;

    /// Returns a reference to `self` if [`Some`], otherwise evaluates `default()`.
    ///
    /// Compared to [`Option::unwrap_or_else`], this saves having to [`Clone::clone`] the value to make the types line up.
    ///
    /// Consider using [`Self::as_ref_or_else`] instead if the type does not implement [`Deref`].
    fn as_deref_or_else(&self, default: impl FnOnce() -> T) -> Cow<T::Target>
    where
        T: Deref,
        T::Target: ToOwned<Owned = T>;
}

impl<T> OptionExt<T> for Option<T> {
    fn as_ref_or_else(&self, default: impl FnOnce() -> T) -> Cow<T>
    where
        T: Clone,
    {
        self.as_ref()
            .map_or_else(|| Cow::Owned(default()), Cow::Borrowed)
    }

    fn as_deref_or_else(&self, default: impl FnOnce() -> T) -> Cow<<T>::Target>
    where
        T: Deref,
        <T>::Target: ToOwned<Owned = T>,
    {
        self.as_deref()
            .map_or_else(|| Cow::Owned(default()), Cow::Borrowed)
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;

    #[test]
    fn as_ref_or_else() {
        let maybe: Option<String> = None;
        let defaulted: Cow<String> = maybe.as_ref_or_else(|| "foo".to_string());
        assert_eq!(defaulted, Cow::<String>::Owned("foo".to_string()));

        let maybe: Option<String> = Some("foo".to_string());
        let defaulted: Cow<String> = maybe.as_ref_or_else(|| panic!());
        assert_eq!(defaulted, Cow::<String>::Borrowed(&"foo".to_string()));
    }

    #[test]
    fn as_deref_or_else() {
        let maybe: Option<String> = None;
        let defaulted: Cow<str> = maybe.as_deref_or_else(|| "foo".to_string());
        assert_eq!(defaulted, Cow::<str>::Owned("foo".to_string()));

        let maybe: Option<String> = Some("foo".to_string());
        let defaulted: Cow<str> = maybe.as_deref_or_else(|| panic!());
        assert_eq!(defaulted, Cow::<str>::Borrowed("foo"));
    }
}
