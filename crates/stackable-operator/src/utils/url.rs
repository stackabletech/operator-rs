use snafu::{Snafu, ensure};

#[derive(Debug, PartialEq, Eq, Snafu)]
pub enum UrlError {
    #[snafu(display(
        "failed to use {segments} as url path segment{plural}",
        segments = segments.join(", "),
        plural = if segments.len() == 1 { "" } else { "s" }
    ))]
    InvalidSegments { segments: Vec<String> },

    #[snafu(display("failed to append path segments to url as it cannot be a base"))]
    CannotBeABase,
}

/// An extension trait for [`url::Url`], most notably providing a safe alternative for
/// [`url::Url::join`]: [`UrlExt::join_safe`].
pub trait UrlExt: Sized {
    /// Joins one or more segments to the end of the existing URL path segments and returns a new
    /// [`url::Url`].
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
    fn join_safe<T>(&self, segments: &[T]) -> Result<Self, UrlError>
    where
        T: AsRef<str>;
}

impl UrlExt for url::Url {
    fn join_safe<T>(&self, segments: &[T]) -> Result<Self, UrlError>
    where
        T: AsRef<str>,
    {
        let invalid_segments = segments
            .iter()
            .map(AsRef::as_ref)
            .filter(|s| matches!(*s, "" | "." | "..") || s.contains(['\t', '\n', '\r']))
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();

        ensure!(
            invalid_segments.is_empty(),
            InvalidSegmentsSnafu {
                segments: invalid_segments
            }
        );

        let mut url = self.clone();
        url.path_segments_mut()
            // A URL cannot be a base when:
            //
            // - its scheme is not one of the "special" schemes (http, https, ws, wss, ftp, file), and
            // - it has no authority (no // after the scheme)
            //
            // Examples: mailto:user@example.com, javascript:alert(1)
            //
            // See https://url.spec.whatwg.org/#cannot-be-a-base-url-path-state
            .map_err(|()| CannotBeABaseSnafu.build())?
            .pop_if_empty()
            .extend(segments);

        Ok(url)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::path_slash("https://example.org/a/", "https://example.org/a/foo/bar/baz")]
    #[case::path("https://example.org/a", "https://example.org/a/foo/bar/baz")]
    #[case::root_slash("https://example.org/", "https://example.org/foo/bar/baz")]
    #[case::root("https://example.org", "https://example.org/foo/bar/baz")]
    fn join_safe_keeps_base_path(#[case] base: &str, #[case] expected: &str) {
        let url = url::Url::parse(base).expect("base URL must parse");
        let url = url
            .join_safe(&["foo", "bar", "baz"])
            .expect("must join, all segments are valid");

        assert_eq!(url.as_str(), expected);
    }

    #[rstest]
    #[case::absolute_url("https://evil.example.com/", "https:%2F%2Fevil.example.com%2F")]
    #[case::scheme_relative("//evil.example.com", "%2F%2Fevil.example.com")]
    #[case::traversal("../../realms/master", "..%2F..%2Frealms%2Fmaster")]
    #[case::percent_encoded_dots("%2e%2e", "%252e%252e")]
    #[case::backslash("..\\admin", "..%5Cadmin")]
    #[case::query("alice?exact=false", "alice%3Fexact=false")]
    #[case::fragment("alice#", "alice%23")]
    #[case::guest_principal_name(
        "alice_example.com#EXT#@example.org",
        "alice_example.com%23EXT%23@example.org"
    )]
    fn join_safe_appends_malicious_segment(#[case] segment: &str, #[case] expected: &str) {
        let url = url::Url::parse("http://example.org").expect("base URL must parse");
        let url = url
            .join_safe(&[segment])
            .expect("must join, all segments are valid");

        assert_eq!(url.as_str(), format!("http://example.org/{expected}"));
    }

    #[rstest]
    #[case::multiple(&["", ".", ".."])]
    #[case::empty(&[""])]
    #[case::current(&["."])]
    #[case::parent(&[".."])]
    #[case::tab(&["ad\tmin"])]
    #[case::line_feed(&["ad\nmin"])]
    #[case::carriage_return(&["ad\rmin"])]
    fn join_safe_rejects_invalid_segment(#[case] segments: &[&str]) {
        let url = url::Url::parse("http://example.org").expect("base URL must parse");
        let error = url
            .join_safe(segments)
            .expect_err("must be error because invalid segments are not appended");

        assert_eq!(
            error,
            UrlError::InvalidSegments {
                // NOTE (@Techassi): We use to_string instead of to_owned (which we normally prefer)
                // because we have a double reference (&&str) here.
                segments: segments.iter().map(ToString::to_string).collect()
            }
        );
    }

    #[test]
    fn join_safe_rejects_invalid_base() {
        let url = url::Url::parse("mailto:user@example.org").expect("base URL must parse");
        let error = url
            .join_safe(&["foo", "bar", "baz"])
            .expect_err("must be error because the base URL cannot be a base (opaque path state)");

        assert_eq!(error, UrlError::CannotBeABase);
    }
}
