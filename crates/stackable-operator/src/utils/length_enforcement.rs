use sha2::{Digest, Sha256};

/// Ensures that the given input does not exceed the given maximum length.
/// If required, the input is truncated and a hex encoded hash is appended with a dash.
///
/// It is recommended to only use ASCII characters, but this function also handles UTF-8: Multi-byte
/// characters are never split up, so the result can be shorter than the maximum length.
///
/// If the truncation does not leave any character then only the hash is returned.
///
/// # Panics
///
/// Panics if `max_length_bytes < 1 /* character */ + 1 /* dash */ + hash_length`.
pub fn ensure_max_length(
    original: impl Into<String>,
    max_length_bytes: usize,
    hash_length: usize,
) -> String {
    assert!(max_length_bytes >= 1 /* character */ + 1 /* dash */ + hash_length);

    let original = original.into();
    if original.len() <= max_length_bytes {
        return original;
    }
    if hash_length == 0 {
        return truncate_at_char_boundary(original, max_length_bytes);
    }

    let mut hash = format!("{:x}", Sha256::digest(original.as_bytes()));
    hash.truncate(hash_length);

    // The result is `<name>-<hash>`, so the name must not occupy the bytes which are reserved
    // for the hash.
    let mut name = truncate_at_char_boundary(original, max_length_bytes - hash_length);

    // Remove one more character to make room for the dash.
    let removed_char = name.pop();

    if name.is_empty() {
        return hash;
    }

    // A dash at the end of the name is reused as the separator. If the removed character was a
    // dash itself then both dashes belong to the name and are kept.
    if !name.ends_with('-') || removed_char == Some('-') {
        name.push('-');
    }

    format!("{name}{hash}")
}

/// Truncates the given input to at most `max_length_bytes` bytes.
///
/// The input is only truncated at a character boundary, so a multi-byte character is never split
/// up but dropped entirely.
fn truncate_at_char_boundary(mut input: String, max_length_bytes: usize) -> String {
    input.truncate(input.floor_char_boundary(max_length_bytes));
    input
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_ensure_max_length() {
        // empty resource name, no hash length
        assert_eq!(String::new(), ensure_max_length(String::new(), 2, 0));

        // resource_name.len() <= max_length
        assert_eq!(
            "abcdef".to_owned(),
            ensure_max_length("abcdef".to_owned(), 6, 4)
        );

        // hash_length == 0
        assert_eq!(
            "abcdef".to_owned(),
            ensure_max_length("abcdefg".to_owned(), 6, 0)
        );

        // hash appended with dash
        assert_eq!(
            "a-7d1a".to_owned(),
            ensure_max_length("abcdefg".to_owned(), 6, 4)
        );

        // hash appended without an extra dash
        assert_eq!(
            "ab-a1b1".to_owned(),
            ensure_max_length("ab-defgh".to_owned(), 7, 4)
        );

        // hash appended without an extra dash
        // In this case, the result is one character shorter than the maximum length.
        assert_eq!(
            "a-3951".to_owned(),
            ensure_max_length("a-cdefgh".to_owned(), 7, 4)
        );

        // hash appended without an extra dash
        // The two dashes in the given resource name are intentionally kept.
        assert_eq!(
            "a--f7a0".to_owned(),
            ensure_max_length("a--defgh".to_owned(), 7, 4)
        );

        // A hash_length longer than the produced hash string may not produce the desired result.
        // Just use sensible values!
        assert_eq!(
            "aaaaaaaaa-d476ce01c3787bcab054a2cf48d6af6dd303a0eb549e21a74125132f79d90c36".to_owned(),
            ensure_max_length("a".repeat(1011), 1010, 1000)
        );
    }

    /// The maximum length is measured in bytes, so multi-byte characters must not be split up by
    /// the truncation. This can make the result shorter than the maximum length.
    #[test]
    fn test_ensure_max_length_with_multi_byte_characters() {
        // The two byte characters fit exactly into the maximum length.
        assert_eq!("äöü".to_owned(), ensure_max_length("äöü".to_owned(), 6, 4));

        // Truncating after 5 bytes would split up the "ü", so it is dropped entirely.
        assert_eq!("äö".to_owned(), ensure_max_length("äöü".to_owned(), 5, 0));

        // The 5 bytes reserved for the name only fit "äö", of which the "ö" is then replaced by
        // the dash, so the result is two bytes shorter than the maximum length.
        assert_eq!(
            "ä-e109".to_owned(),
            ensure_max_length("äöüäöü".to_owned(), 9, 4)
        );

        // hash appended with dash, three byte characters
        assert_eq!(
            "日-9efa".to_owned(),
            ensure_max_length("日本語日本語".to_owned(), 10, 4)
        );

        // hash appended with dash, four byte characters
        assert_eq!(
            "🚀-a13c".to_owned(),
            ensure_max_length("🚀🚀🚀🚀".to_owned(), 13, 4)
        );

        // The trailing dash of the truncated name is replaced by the dash which separates the
        // hash.
        assert_eq!(
            "aä-f726".to_owned(),
            ensure_max_length("aä-öüb".to_owned(), 8, 4)
        );

        // The truncated name is "aä-ö", so the "ö" is dropped and the existing dash is reused.
        assert_eq!(
            "aä-ae0c".to_owned(),
            ensure_max_length("aä-öüäöü".to_owned(), 10, 4)
        );

        // The truncation does not leave any character, so only the hash is returned.
        assert_eq!(
            "d24d".to_owned(),
            ensure_max_length("🚀🚀🚀".to_owned(), 6, 4)
        );
    }
}
