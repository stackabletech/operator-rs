use snafu::Snafu;

#[derive(Debug, Snafu)]
pub enum UrlError {
    #[snafu(display("failed to use {segment:?} as url path segment"))]
    InvalidSegment { segment: String },

    #[snafu(display("failed to append path segments to url as it cannot be a base"))]
    CannotBeABase,
}

pub trait UrlExt: Sized {
    /// Joins one or more segments to the end of the existing URL path segments.
    ///
    /// Compared to [`url::Url::join`], it protects users against (accidental) misuse (especially
    /// when dealing with untrusted input) in two ways:
    ///
    /// 1. The function validates that each segment is not an invalid segment. This for example
    ///    prevents path traversals.
    /// 2. The upstream function can replace the entire base URL in these two situations:
    ///    - A [scheme relative special URL][1] as input replaces everything in the base URL after
    ///      the scheme.
    ///    - An absolute URL (with a scheme) as input replaces the whole base URL (even the
    ///      scheme).
    ///
    ///    _From <https://docs.rs/url/2.5.8/url/struct.Url.html#method.join> (Accessed 2026-10-19,
    ///    version 2.5.8)_
    ///
    ///    This for example allows users to construct malicious URLs. This function constructs the
    ///    final URL in a way in which the segments are only appended to any existing path segments.
    ///
    /// [1]: https://url.spec.whatwg.org/#scheme-relative-special-url-string
    fn join_safe<T>(&mut self, segments: &[T]) -> Result<(), UrlError>
    where
        T: AsRef<str>;
}

impl UrlExt for url::Url {
    fn join_safe<T>(&mut self, segments: &[T]) -> Result<(), UrlError>
    where
        T: AsRef<str>,
    {
        let invalid_segment = segments.iter().find(|segment| {
            let segment = segment.as_ref();
            matches!(segment, "" | "." | "..") || segment.contains(['\t', '\n', '\r'])
        });

        if let Some(segment) = invalid_segment {
            return InvalidSegmentSnafu {
                segment: segment.as_ref(),
            }
            .fail();
        }

        self.path_segments_mut()
            .map_err(|()| CannotBeABaseSnafu.build())?
            .pop_if_empty()
            .extend(segments);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::path_slash("http://example.com/a/", "http://example.com/a/foo/bar/baz")]
    #[case::path("http://example.com/a", "http://example.com/a/foo/bar/baz")]
    #[case::root_slash("http://example.com/", "http://example.com/foo/bar/baz")]
    #[case::root("http://example.com", "http://example.com/foo/bar/baz")]
    fn join_safe_keeps_base_path(#[case] base: &str, #[case] expected: &str) {
        let mut url = url::Url::parse(base).expect("base URL must parse");
        url.join_safe(&["foo", "bar", "baz"])
            .expect("must join, all segments are valid");

        assert_eq!(url.as_str(), expected);
    }
}
