//! `flux-converter` is part of the project DeLorean :)
//!
//! It converts between different CRD versions by using 1.21 GW of power,
//! 142km/h and time travel.

use std::fmt::Display;

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub struct UnknownResourceVersionError {
    pub version: String,
}

impl std::error::Error for UnknownResourceVersionError {}
impl Display for UnknownResourceVersionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "The version {version} is not known",
            version = self.version
        )
    }
}
