pub trait UrlExt: Sized {
    /// Joins many path segments to the [`Url`][url::Url]. Note: Passing
    /// segments without a trailing slash while not being the final segment
    /// will **allocate** on the heap to append the missing trailing slash.
    /// See <https://docs.rs/url/latest/url/struct.Url.html#method.join> for
    /// more information about why the trailing slash is important.
    ///
    /// ### Example
    ///
    /// ```
    /// use stackable_operator::utils::UrlExt;
    ///
    /// let url = url::Url::parse("http://example.com").unwrap();
    /// let url = url.join_many(vec!["realms/", "master/", "myuser"]).unwrap();
    ///
    /// assert_eq!(url.as_str(), "http://example.com/realms/master/myuser");
    /// ```
    fn join_many<'a>(
        self,
        inputs: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, url::ParseError>;
}

impl UrlExt for url::Url {
    fn join_many<'a>(
        self,
        inputs: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, url::ParseError> {
        let mut iter = inputs.into_iter().peekable();
        let mut url = self;

        while let Some(input) = iter.next() {
            url = if !input.ends_with('/') && iter.peek().is_some() {
                url.join(&format!("{}/", input))?
            } else {
                url.join(input)?
            };
        }

        Ok(url)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn url_join_many() {
        let url = url::Url::parse("http://example.com").unwrap();
        let url = url.join_many(vec!["realms/", "master/", "myuser"]).unwrap();

        assert_eq!(url.as_str(), "http://example.com/realms/master/myuser");
    }
}
