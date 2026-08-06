use sha2::{Digest, Sha256};

/// Ensures that the given input does not exceed the given maximum length.
/// If required, the input is truncated and a hex encoded hash is appended with a dash.
///
/// # Panics
///
/// Panics if `max_length < 1 /* character */ + 1 /* dash */ + hash_length`.
pub fn ensure_max_length(
    original: impl Into<String>,
    max_length: usize,
    hash_length: usize,
) -> String {
    assert!(max_length >= 1 /* character */ + 1 /* dash */ + hash_length);

    let original = original.into();
    if original.len() <= max_length {
        original
    } else if hash_length == 0 {
        let mut truncated_name = original;
        truncated_name.truncate(max_length);
        truncated_name
    } else {
        let mut hash = format!("{:x}", Sha256::digest(original.as_bytes()));
        hash.truncate(hash_length);

        let mut truncated_name = original;
        // Truncate the name so that the hash can be appended without exceeding the maximum
        // length.
        truncated_name.truncate(max_length - hash_length);

        let last_char = truncated_name
            .pop()
            .expect("should be guaranteed by the assertion above");
        let second_to_last_char = truncated_name
            .pop()
            .expect("should be guaranteed by the assertion above");

        // If the truncated name already ends with a dash then do not add another one,
        // otherwise replace the last character with a dash.
        if second_to_last_char == '-' && last_char != '-' {
            format!("{truncated_name}{second_to_last_char}{hash}")
        } else {
            format!("{truncated_name}{second_to_last_char}-{hash}")
        }
    }
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
}
