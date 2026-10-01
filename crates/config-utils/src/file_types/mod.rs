use std::{collections::HashMap, sync::LazyLock};

use clap::ValueEnum;
use properties::PropertiesEscaper;
use xml::XmlEscaper;

mod properties;
mod xml;

// Yes, we could use `strum` for that, but we try to keep the dependencies minimal.
pub static KNOWN_FILE_TYPES: LazyLock<HashMap<String, FileType>> = LazyLock::new(|| {
    let mut types = HashMap::new();
    types.insert("properties".to_owned(), FileType::Properties);
    types.insert("xml".to_owned(), FileType::Xml);
    types
});

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum FileType {
    Properties,
    Xml,
}

pub trait Escape {
    fn escape(line: String) -> String;
}

impl FileType {
    pub fn escape(&self, line: String) -> String {
        match self {
            FileType::Properties => PropertiesEscaper::escape(line),
            FileType::Xml => XmlEscaper::escape(line),
        }
    }
}
