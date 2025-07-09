#[cfg(doc)]
use std::path::PathBuf;
use std::{borrow::Cow, ops::Deref};

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
        let maybe: Option<&str> = None;
        let defaulted: Cow<&str> = maybe.as_ref_or_else(|| "foo");
        assert_eq!(defaulted, Cow::<&str>::Owned("foo"));

        let maybe: Option<&str> = Some("foo");
        let defaulted: Cow<&str> = maybe.as_ref_or_else(|| panic!());
        assert_eq!(defaulted, Cow::<&str>::Borrowed(&"foo"));
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
