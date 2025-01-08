use std::{
    fmt::{Display, Write},
    num::ParseFloatError,
    str::FromStr,
};

use k8s_openapi::apimachinery::pkg::api::resource::Quantity as K8sQuantity;
use snafu::{ensure, ResultExt as _, Snafu};

mod cpu;
mod macros;
mod memory;
mod ops;
mod suffix;

pub use cpu::*;
pub use memory::*;
pub use suffix::*;

#[derive(Debug, PartialEq, Snafu)]
pub enum ParseQuantityError {
    #[snafu(display("input is either empty or contains non-ascii characters"))]
    InvalidFormat,

    #[snafu(display("failed to parse floating point number"))]
    InvalidFloat { source: ParseFloatError },

    #[snafu(display("failed to parse suffix"))]
    InvalidSuffix { source: ParseSuffixError },
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Quantity {
    // FIXME (@Techassi): Support arbitrary-precision numbers
    /// The numeric value of the quantity.
    ///
    /// This field holds data parsed from `<signedNumber>` according to the spec. We especially opt
    /// to not use arbitrary-precision arithmetic like the Go implementation, as we don't see the
    /// need to support these huge numbers.
    value: f64,

    /// The suffix of the quantity.
    ///
    /// This field holds data parsed from `<suffix>` according to the spec.
    suffix: Suffix,
}

impl FromStr for Quantity {
    type Err = ParseQuantityError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        ensure!(!input.is_empty() && input.is_ascii(), InvalidFormatSnafu);

        if input == "0" {
            return Ok(Self {
                suffix: Suffix::DecimalMultiple(DecimalMultiple::Empty),
                value: 0.0,
            });
        }

        match input.find(|c: char| c != '.' && !c.is_ascii_digit()) {
            Some(suffix_index) => {
                let parts = input.split_at(suffix_index);
                let value = f64::from_str(parts.0).context(InvalidFloatSnafu)?;
                let suffix = Suffix::from_str(parts.1).context(InvalidSuffixSnafu)?;

                Ok(Self { suffix, value })
            }
            None => {
                let value = f64::from_str(input).context(InvalidFloatSnafu)?;

                Ok(Self {
                    suffix: Suffix::DecimalMultiple(DecimalMultiple::Empty),
                    value,
                })
            }
        }
    }
}

impl Display for Quantity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.value == 0.0 {
            return f.write_char('0');
        }

        write!(
            f,
            "{value}{suffix}",
            value = self.value,
            suffix = self.suffix
        )
    }
}

impl From<Quantity> for K8sQuantity {
    fn from(value: Quantity) -> Self {
        K8sQuantity(value.to_string())
    }
}

impl From<&Quantity> for K8sQuantity {
    fn from(value: &Quantity) -> Self {
        K8sQuantity(value.to_string())
    }
}

impl TryFrom<K8sQuantity> for Quantity {
    type Error = ParseQuantityError;

    fn try_from(value: K8sQuantity) -> Result<Self, Self::Error> {
        Quantity::from_str(&value.0)
    }
}

impl TryFrom<&K8sQuantity> for Quantity {
    type Error = ParseQuantityError;

    fn try_from(value: &K8sQuantity) -> Result<Self, Self::Error> {
        Quantity::from_str(&value.0)
    }
}

impl Quantity {
    /// Optionally scales up or down to the provided `suffix`.
    ///
    /// No scaling is performed in the following cases:
    ///
    /// - the suffixes already match
    /// - the value is 0
    pub fn scale_to(self, suffix: Suffix) -> Self {
        match (self.value, &self.suffix) {
            (0.0, _) => self,
            (_, s) if *s == suffix => self,
            (v, s) => {
                let factor = s.factor() / suffix.factor();

                Self {
                    value: v * factor,
                    suffix,
                }
            }
        }
    }

    /// Either sets the suffix of `self` to `rhs` or scales `rhs` if `self` has a value other than
    /// zero.
    ///
    /// This function is currently used for the [`std::ops::Add`] and [`std::ops::Sub`]
    /// implementations.
    pub fn set_suffix_or_scale_rhs(self, rhs: Self) -> (Self, Self) {
        if self.value == 0.0 {
            (
                Self {
                    suffix: rhs.suffix,
                    ..self
                },
                rhs,
            )
        } else {
            (self, rhs.scale_to(self.suffix))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("49041204Ki", Quantity { value: 49041204.0, suffix: Suffix::BinaryMultiple(BinaryMultiple::Kibi) })]
    #[case("256Ki", Quantity { value: 256.0, suffix: Suffix::BinaryMultiple(BinaryMultiple::Kibi) })]
    #[case("1.5Gi", Quantity { value: 1.5, suffix: Suffix::BinaryMultiple(BinaryMultiple::Gibi) })]
    #[case("0.8Ti", Quantity { value: 0.8, suffix: Suffix::BinaryMultiple(BinaryMultiple::Tebi) })]
    #[case("3.2Pi", Quantity { value: 3.2, suffix: Suffix::BinaryMultiple(BinaryMultiple::Pebi) })]
    #[case("0.2Ei", Quantity { value: 0.2, suffix: Suffix::BinaryMultiple(BinaryMultiple::Exbi) })]
    #[case("8Mi", Quantity { value: 8.0, suffix: Suffix::BinaryMultiple(BinaryMultiple::Mebi) })]
    fn binary_quantity_from_str_pass(#[case] input: &str, #[case] expected: Quantity) {
        let parsed = Quantity::from_str(input).unwrap();
        assert_eq!(parsed, expected);
    }

    #[rstest]
    #[case("49041204k", Quantity { value: 49041204.0, suffix: Suffix::DecimalMultiple(DecimalMultiple::Kilo) })]
    #[case("256k", Quantity { value: 256.0, suffix: Suffix::DecimalMultiple(DecimalMultiple::Kilo) })]
    #[case("1.5G", Quantity { value: 1.5, suffix: Suffix::DecimalMultiple(DecimalMultiple::Giga) })]
    #[case("0.8T", Quantity { value: 0.8, suffix: Suffix::DecimalMultiple(DecimalMultiple::Tera) })]
    #[case("3.2P", Quantity { value: 3.2, suffix: Suffix::DecimalMultiple(DecimalMultiple::Peta) })]
    #[case("0.2E", Quantity { value: 0.2, suffix: Suffix::DecimalMultiple(DecimalMultiple::Exa) })]
    #[case("4m", Quantity { value: 4.0, suffix: Suffix::DecimalMultiple(DecimalMultiple::Milli) })]
    #[case("8M", Quantity { value: 8.0, suffix: Suffix::DecimalMultiple(DecimalMultiple::Mega) })]
    fn decimal_quantity_from_str_pass(#[case] input: &str, #[case] expected: Quantity) {
        let parsed = Quantity::from_str(input).unwrap();
        assert_eq!(parsed, expected);
    }

    #[rstest]
    #[case("1.234e-3.21", Quantity { value: 1.234, suffix: Suffix::DecimalExponent(DecimalExponent::from(-3.21)) })]
    #[case("1.234E-3.21", Quantity { value: 1.234, suffix: Suffix::DecimalExponent(DecimalExponent::from(-3.21)) })]
    #[case("1.234e3", Quantity { value: 1.234, suffix: Suffix::DecimalExponent(DecimalExponent::from(3.0)) })]
    #[case("1.234E3", Quantity { value: 1.234, suffix: Suffix::DecimalExponent(DecimalExponent::from(3.0)) })]
    fn decimal_exponent_quantity_from_str_pass(#[case] input: &str, #[case] expected: Quantity) {
        let parsed = Quantity::from_str(input).unwrap();
        assert_eq!(parsed, expected);
    }

    #[rstest]
    #[case("0Mi", Some("0"))]
    #[case("256Ki", None)]
    #[case("1.5Gi", None)]
    #[case("0.8Ti", None)]
    #[case("3.2Pi", None)]
    #[case("0.2Ei", None)]
    #[case("8Mi", None)]
    #[case("0", None)]
    fn binary_to_string_pass(#[case] input: &str, #[case] output: Option<&str>) {
        let parsed = Quantity::from_str(input).unwrap();
        assert_eq!(output.unwrap_or(input), parsed.to_string());
    }

    #[rstest]
    #[case("1Mi", BinaryMultiple::Kibi, "1024Ki")]
    #[case("1024Ki", BinaryMultiple::Mebi, "1Mi")]
    #[case("1Mi", BinaryMultiple::Mebi, "1Mi")]
    fn binary_to_binary_scale_pass(
        #[case] input: &str,
        #[case] scale_to: BinaryMultiple,
        #[case] output: &str,
    ) {
        let parsed = Quantity::from_str(input)
            .unwrap()
            .scale_to(Suffix::BinaryMultiple(scale_to));

        assert_eq!(parsed.to_string(), output);
    }

    #[rstest]
    #[case("1Mi", DecimalMultiple::Kilo, "1048.576k")]
    #[case("1Mi", DecimalMultiple::Mega, "1.048576M")]
    fn binary_to_decimal_scale_pass(
        #[case] input: &str,
        #[case] scale_to: DecimalMultiple,
        #[case] output: &str,
    ) {
        let parsed = Quantity::from_str(input)
            .unwrap()
            .scale_to(Suffix::DecimalMultiple(scale_to));

        assert_eq!(parsed.to_string(), output);
    }

    #[rstest]
    #[case("1M", DecimalMultiple::Kilo, "1000k")]
    #[case("1000k", DecimalMultiple::Mega, "1M")]
    #[case("1M", DecimalMultiple::Mega, "1M")]
    fn decimal_to_decimal_scale_pass(
        #[case] input: &str,
        #[case] scale_to: DecimalMultiple,
        #[case] output: &str,
    ) {
        let parsed = Quantity::from_str(input)
            .unwrap()
            .scale_to(Suffix::DecimalMultiple(scale_to));

        assert_eq!(parsed.to_string(), output);
    }

    #[rstest]
    #[case("1e3", DecimalExponent::from(0.0), "1000e0")]
    #[case("1000e0", DecimalExponent::from(3.0), "1e3")]
    #[case("1e3", DecimalExponent::from(3.0), "1e3")]
    fn decimal_exponent_to_decimal_exponent_scale_pass(
        #[case] input: &str,
        #[case] scale_to: DecimalExponent,
        #[case] output: &str,
    ) {
        let parsed = Quantity::from_str(input)
            .unwrap()
            .scale_to(Suffix::DecimalExponent(scale_to));

        assert_eq!(parsed.to_string(), output);
    }
}
