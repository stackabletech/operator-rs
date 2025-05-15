//! `flux-converter` is part of the project DeLorean :)
//!
//! It converts between different CRD versions by using 1.21 GW of power,
//! 142km/h and time travel.

use snafu::Snafu;

#[cfg(test)]
mod tests;

#[derive(Debug, Snafu)]
pub enum ParseResourceVersionError {
    #[snafu(display("The resource version \"{version}\" is not known"))]
    UnknownResourceVersion { version: String },
}
