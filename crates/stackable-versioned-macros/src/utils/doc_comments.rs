pub trait DocComments {
    /// Converts lines of doc-comments into a trimmed list which can be expanded via repetition in
    /// [`quote::quote`].
    fn into_doc_comments(self) -> Vec<String>;
}

impl DocComments for &str {
    fn into_doc_comments(self) -> Vec<String> {
        self
            // Trim the leading and trailing whitespace, deleting superfluous
            // empty lines.
            .trim()
            .lines()
            // Trim the leading and trailing whitespace on each line that can be
            // introduced when the developer indents multi-line comments.
            .map(|line| line.trim().to_owned())
            .collect()
    }
}

impl DocComments for Option<&str> {
    fn into_doc_comments(self) -> Vec<String> {
        self.map_or(vec![], |s| s.into_doc_comments())
    }
}
