use super::Escape;

pub struct PropertiesEscaper;

// https://docs.oracle.com/javase/6/docs/api/java/util/Properties.html#load(java.io.Reader)
impl Escape for PropertiesEscaper {
    fn escape(line: String) -> String {
        // Copied from https://github.com/adamcrume/java-properties/blob/0335bfb733444e0b9326405bc7845be449bec1f3/src/lib.rs#L809
        let mut escaped = String::new();
        for c in line.chars() {
            match c {
                '\\' => escaped.push_str("\\\\"),
                ' ' => escaped.push_str("\\ "),
                '\t' => escaped.push_str("\\t"),
                '\r' => escaped.push_str("\\r"),
                '\n' => escaped.push_str("\\n"),
                '\x0c' => escaped.push_str("\\f"),
                ':' => escaped.push_str("\\:"),
                '=' => escaped.push_str("\\="),
                '!' => escaped.push_str("\\!"),
                '#' => escaped.push_str("\\#"),
                _ if c < ' ' => escaped.push_str(&format!("\\u{:x}", c as u16)),
                _ => escaped.push(c),
            }
        }

        escaped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rstest::rstest;

    #[rstest]
    #[case("foo", "foo")]
    #[case("foo bar", "foo\\ bar")]
    #[case("<foo> bar", "<foo>\\ bar")]
    #[case("foo<>'\"&\r\nbar", "foo<>'\"&\\r\\nbar")]
    fn test_xml_escaping(#[case] input: String, #[case] expected: String) {
        assert_eq!(PropertiesEscaper::escape(input), expected);
    }
}
