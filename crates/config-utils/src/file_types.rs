use std::collections::HashMap;

use clap::ValueEnum;
use lazy_static::lazy_static;

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
