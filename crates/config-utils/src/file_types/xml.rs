use super::Escape;

pub struct XmlEscaper;

impl Escape for XmlEscaper {
    fn escape(_line: String) -> String {
        todo!("Implement escaping for XML files")
    }
}
