use std::str::FromStr;

use crate::error::{Error, OperatorResult};
use tracing::info;

/// Prints helpful and standardized diagnostic messages.
///
/// This method is meant to be called first thing in the `main` method of an Operator.
///
/// # Usage
///
/// Use the [`built`](https://crates.io/crates/built) crate and include it in your `main.rs` like this:
///
/// ```text
/// mod built_info {
///     // The file has been placed there by the build script.
///     include!(concat!(env!("OUT_DIR"), "/built.rs"));
/// }
/// ```
///
/// Then call this method in your `main` method:
///
/// ```text
/// stackable_operator::utils::print_startup_string(
///      built_info::PKG_DESCRIPTION,
///      built_info::PKG_VERSION,
///      built_info::GIT_VERSION,
///      built_info::TARGET,
///      built_info::BUILT_TIME_UTC,
///      built_info::RUSTC_VERSION,
/// );
/// ```
pub fn print_startup_string(
    pkg_description: &str,
    pkg_version: &str,
    git_version: Option<&str>,
    target: &str,
    built_time: &str,
    rustc_version: &str,
) {
    let git_information = match git_version {
        None => "".to_string(),
        Some(git) => format!(" (Git information: {})", git),
    };
    info!("Starting {}", pkg_description);
    info!(
        "This is version {}{}, built for {} by {} at {}",
        pkg_version, git_information, target, rustc_version, built_time
    )
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
enum BinaryMultiple {
    Kibi,
    Mebi,
    Gibi,
    Tebi,
    Pebi,
    Exbi,
}

impl BinaryMultiple {
    pub fn to_legacy(&self) -> String {
        match self {
            BinaryMultiple::Kibi => "k".to_string(),
            BinaryMultiple::Mebi => "m".to_string(),
            BinaryMultiple::Gibi => "g".to_string(),
            BinaryMultiple::Tebi => "t".to_string(),
            BinaryMultiple::Pebi => "p".to_string(),
            BinaryMultiple::Exbi => "e".to_string(),
        }
    }

    pub fn upscale(&self) -> Self {
        match self {
            BinaryMultiple::Kibi => BinaryMultiple::Kibi,
            BinaryMultiple::Mebi => BinaryMultiple::Kibi,
            BinaryMultiple::Gibi => BinaryMultiple::Mebi,
            BinaryMultiple::Tebi => BinaryMultiple::Gibi,
            BinaryMultiple::Pebi => BinaryMultiple::Tebi,
            BinaryMultiple::Exbi => BinaryMultiple::Pebi,
        }
    }
}

impl FromStr for BinaryMultiple {
    type Err = Error;

    fn from_str(q: &str) -> OperatorResult<BinaryMultiple> {
        let lq = q.to_lowercase();
        match lq.as_str() {
            "ki" | "kib" => Ok(BinaryMultiple::Kibi),
            "mi" | "mib" => Ok(BinaryMultiple::Mebi),
            "gi" | "gib" => Ok(BinaryMultiple::Gibi),
            "ti" | "tib" => Ok(BinaryMultiple::Tebi),
            "pi" | "pib" => Ok(BinaryMultiple::Pebi),
            "ei" | "eib" => Ok(BinaryMultiple::Exbi),
            _ => Err(Error::InvalidQuantityUnit {
                value: q.to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct QuantityAndUnit {
    value: f32,
    unit: BinaryMultiple,
}

impl QuantityAndUnit {
    pub fn scale(&self, factor: f32) -> Self {
        if factor < 1.0 && self.unit != BinaryMultiple::Kibi {
            QuantityAndUnit {
                value: self.value * factor * 1024.0,
                unit: self.unit.upscale(),
            }
        } else {
            QuantityAndUnit {
                value: self.value * factor,
                unit: self.unit.clone(),
            }
        }
    }

    fn to_java_heap(&self, factor: f32) -> String {
        let scaled = self.scale(factor);
        format!("-Xmx{:.0}{}", scaled.value, scaled.unit.to_legacy())
    }
}

impl FromStr for QuantityAndUnit {
    type Err = Error;

    fn from_str(q: &str) -> OperatorResult<QuantityAndUnit> {
        let mut v = String::from("");
        let mut u = String::from("");

        for c in q.chars() {
            if c.is_numeric() || c == '.' {
                v.push(c);
            } else {
                u.push(c);
            }
        }
        Ok(QuantityAndUnit {
            value: v.parse::<f32>().map_err(|_| Error::InvalidQuantity {
                value: q.to_owned(),
            })?,
            unit: u.parse()?,
        })
    }
}

#[cfg(test)]
mod test {
    use k8s_openapi::apimachinery::pkg::api::resource::Quantity;

    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("256ki", QuantityAndUnit { value: 256f32, unit: BinaryMultiple::Kibi })]
    #[case("8Mib", QuantityAndUnit { value: 8f32, unit: BinaryMultiple::Mebi })]
    #[case("1.5Gi", QuantityAndUnit { value: 1.5f32, unit: BinaryMultiple::Gibi })]
    #[case("0.8tib", QuantityAndUnit { value: 0.8f32, unit: BinaryMultiple::Tebi })]
    #[case("3.2Pi", QuantityAndUnit { value: 3.2f32, unit: BinaryMultiple::Pebi })]
    #[case("0.2ei", QuantityAndUnit { value: 0.2f32, unit: BinaryMultiple::Exbi })]
    pub fn test_quantity_parse(#[case] input: &str, #[case] output: QuantityAndUnit) {
        let got = input.parse::<QuantityAndUnit>().unwrap();
        assert_eq!(got, output);
    }

    #[rstest]
    #[case("256ki", 1.0, "-Xmx256k")]
    #[case("256ki", 0.8, "-Xmx205k")]
    #[case("2mib", 0.8, "-Xmx1638k")]
    #[case("1.5GiB", 0.8, "-Xmx1229m")]
    pub fn test_quantity_scale(#[case] q: &str, #[case] factor: f32, #[case] heap: &str) {
        let qu: QuantityAndUnit = Quantity(q.to_owned()).0.parse().unwrap();
        assert_eq!(heap, qu.to_java_heap(factor));
    }
}
