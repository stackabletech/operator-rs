use std::collections::HashMap;

use clap::ValueEnum;
use lazy_static::lazy_static;
use properties::PropertiesEscaper;
use xml::XmlEscaper;

mod properties;
mod xml;

lazy_static! {
    // Yes, we could use `strum` for that, but we try to keep the dependencies minimal.
    pub static ref KNOWN_FILE_TYPES: HashMap<String, FileType> = {
        let mut types = HashMap::new();
        types.insert("properties".to_owned(), FileType::Properties);
        types.insert("xml".to_owned(), FileType::Xml);
        types
    };
}

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
