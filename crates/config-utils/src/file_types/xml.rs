use super::Escape;

pub struct XmlEscaper;

impl Escape for XmlEscaper {
    fn escape(line: String) -> String {
        let mut escaped = String::new();
        for c in line.chars() {
            match c {
                '<' => escaped.push_str("&lt;"),
                '>' => escaped.push_str("&gt;"),
                '"' => escaped.push_str("&quot;"),
                '\'' => escaped.push_str("&apos;"),
                '&' => escaped.push_str("&amp;"),
                '\n' => escaped.push_str("&#xA;"),
                '\r' => escaped.push_str("&#xD;"),
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
    #[case("foo bar", "foo bar")]
    #[case("<foo> bar", "&lt;foo&gt; bar")]
    #[case("foo<>'\"&\r\nbar", "foo&lt;&gt;&apos;&quot;&amp;&#xD;&#xA;bar")]
    fn test_xml_escaping(#[case] input: String, #[case] expected: String) {
        assert_eq!(XmlEscaper::escape(input), expected);
    }
}
