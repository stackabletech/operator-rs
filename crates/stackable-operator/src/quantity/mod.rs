use std::{
    fmt::{Display, Write},
    num::ParseFloatError,
    ops::Deref,
    str::FromStr,
};

use k8s_openapi::apimachinery::pkg::api::resource::Quantity as K8sQuantity;
use snafu::{ensure, ResultExt as _, Snafu};

mod cpu;
mod macros;
mod memory;
mod ops;

pub use cpu::*;
pub use memory::*;

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
    /// This function returns a value pair which contains an optional [`Quantity`] and a bool
    /// indicating if the function performed any scaling. It returns `false` in the following cases:
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
}

#[derive(Debug, PartialEq, Snafu)]
#[snafu(display("failed to parse {input:?} as quantity suffix"))]
pub struct ParseSuffixError {
    input: String,
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub enum Suffix {
    DecimalMultiple(DecimalMultiple),
    BinaryMultiple(BinaryMultiple),
    DecimalExponent(DecimalExponent),
}

impl FromStr for Suffix {
    type Err = ParseSuffixError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if let Ok(binary_si) = BinaryMultiple::from_str(input) {
            return Ok(Self::BinaryMultiple(binary_si));
        }

        if let Ok(decimal_si) = DecimalMultiple::from_str(input) {
            return Ok(Self::DecimalMultiple(decimal_si));
        }

        if input.starts_with(['e', 'E']) {
            if let Ok(decimal_exponent) = f64::from_str(&input[1..]) {
                return Ok(Self::DecimalExponent(DecimalExponent(decimal_exponent)));
            }
        }

        ParseSuffixSnafu { input }.fail()
    }
}

impl Display for Suffix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Suffix::DecimalMultiple(decimal) => write!(f, "{decimal}"),
            Suffix::BinaryMultiple(binary) => write!(f, "{binary}"),
            Suffix::DecimalExponent(float) => write!(f, "e{float}"),
        }
    }
}

impl Suffix {
    pub fn factor(&self) -> f64 {
        match self {
            Suffix::DecimalMultiple(s) => s.factor(),
            Suffix::BinaryMultiple(s) => s.factor(),
            Suffix::DecimalExponent(s) => s.factor(),
        }
    }
}

/// Supported byte-multiples based on powers of 2.
///
/// These units are defined in IEC 80000-13 and are supported by other standards bodies like NIST.
/// The following list contains examples using the official units which Kubernetes adopted with
/// slight changes (mentioned in parentheses).
///
/// ```plain
/// - 1024^1, KiB (Ki), Kibibyte
/// - 1024^2, MiB (Mi), Mebibyte
/// - 1024^3, GiB (Gi), Gibibyte
/// - 1024^4, TiB (Ti), Tebibyte
/// - 1024^5, PiB (Pi), Pebibyte
/// - 1024^6, EiB (Ei), Exbibyte
/// ```
///
/// All units bigger than Exbibyte are not a valid suffix according to the [Kubernetes serialization
/// format][k8s-serialization-format].
///
/// ### See
///
/// - <https://en.wikipedia.org/wiki/Byte#Multiple-byte_units>
/// - <https://physics.nist.gov/cuu/Units/binary.html>
///
/// [k8s-serialization-format]: https://github.com/kubernetes/apimachinery/blob/8c60292e48e46c4faa1e92acb232ce6adb37512c/pkg/api/resource/quantity.go#L37-L59
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, strum::Display, strum::EnumString)]
pub enum BinaryMultiple {
    #[strum(serialize = "Ki")]
    Kibi,

    #[strum(serialize = "Mi")]
    Mebi,

    #[strum(serialize = "Gi")]
    Gibi,

    #[strum(serialize = "Ti")]
    Tebi,

    #[strum(serialize = "Pi")]
    Pebi,

    #[strum(serialize = "Ei")]
    Exbi,
}

impl BinaryMultiple {
    /// Returns the factor based on powers of 2.
    pub fn factor(&self) -> f64 {
        match self {
            BinaryMultiple::Kibi => 2f64.powi(10),
            BinaryMultiple::Mebi => 2f64.powi(20),
            BinaryMultiple::Gibi => 2f64.powi(30),
            BinaryMultiple::Tebi => 2f64.powi(40),
            BinaryMultiple::Pebi => 2f64.powi(50),
            BinaryMultiple::Exbi => 2f64.powi(60),
        }
    }
}

/// Supported byte-multiples based on powers of 10.
///
/// These units are recommended by the International Electrotechnical Commission (IEC). The
/// following list contains examples using the official SI units and the units used by Kubernetes
/// (mentioned in parentheses). Units used by Kubernetes are a shortened version of the SI units.
///
/// It should also be noted that there is an inconsistency in the format Kubernetes uses. Kilobytes
/// should use 'K' instead of 'k'.
///
/// ```plain
/// - 1000^-1,    (m): millibyte (Kubernetes only)
/// - 1000^ 0,  B ( ): byte      (no suffix)
/// - 1000^ 1, kB (k): kilobyte
/// - 1000^ 2, MB (M): Megabyte
/// - 1000^ 3, GB (G): Gigabyte
/// - 1000^ 4, TB (T): Terabyte
/// - 1000^ 5, PB (P): Petabyte
/// - 1000^ 6, EB (E): Exabyte
/// ```
///
/// All units bigger than Exabyte are not a valid suffix according to the [Kubernetes serialization
/// format][k8s-serialization-format].
///
/// ### See
///
/// - <https://en.wikipedia.org/wiki/Byte#Multiple-byte_units>
/// - <https://physics.nist.gov/cuu/Units/binary.html>
///
/// [k8s-serialization-format]: https://github.com/kubernetes/apimachinery/blob/8c60292e48e46c4faa1e92acb232ce6adb37512c/pkg/api/resource/quantity.go#L37-L59
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, strum::Display, strum::EnumString)]
pub enum DecimalMultiple {
    #[strum(serialize = "m")]
    Milli,

    #[strum(serialize = "")]
    Empty,

    #[strum(serialize = "k")]
    Kilo,

    #[strum(serialize = "M")]
    Mega,

    #[strum(serialize = "G")]
    Giga,

    #[strum(serialize = "T")]
    Tera,

    #[strum(serialize = "P")]
    Peta,

    #[strum(serialize = "E")]
    Exa,
}

impl DecimalMultiple {
    pub fn factor(&self) -> f64 {
        match self {
            DecimalMultiple::Milli => 10f64.powi(-3),
            DecimalMultiple::Empty => 10f64.powi(0),
            DecimalMultiple::Kilo => 10f64.powi(3),
            DecimalMultiple::Mega => 10f64.powi(6),
            DecimalMultiple::Giga => 10f64.powi(9),
            DecimalMultiple::Tera => 10f64.powi(12),
            DecimalMultiple::Peta => 10f64.powi(15),
            DecimalMultiple::Exa => 10f64.powi(18),
        }
    }
}

/// Scientific (also known as E) notation of numbers.
///
/// ### See
///
/// - <https://en.wikipedia.org/wiki/Scientific_notation#E_notation>
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct DecimalExponent(f64);

impl Deref for DecimalExponent {
    type Target = f64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for DecimalExponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl DecimalExponent {
    pub fn factor(&self) -> f64 {
        10f64.powf(self.0)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("Ki", Suffix::BinaryMultiple(BinaryMultiple::Kibi))]
    #[case("Mi", Suffix::BinaryMultiple(BinaryMultiple::Mebi))]
    #[case("Gi", Suffix::BinaryMultiple(BinaryMultiple::Gibi))]
    #[case("Ti", Suffix::BinaryMultiple(BinaryMultiple::Tebi))]
    #[case("Pi", Suffix::BinaryMultiple(BinaryMultiple::Pebi))]
    #[case("Ei", Suffix::BinaryMultiple(BinaryMultiple::Exbi))]
    fn binary_multiple_from_str_pass(#[case] input: &str, #[case] expected: Suffix) {
        let parsed = Suffix::from_str(input).unwrap();
        assert_eq!(parsed, expected);
    }

    #[rstest]
    #[case("m", Suffix::DecimalMultiple(DecimalMultiple::Milli))]
    #[case("", Suffix::DecimalMultiple(DecimalMultiple::Empty))]
    #[case("k", Suffix::DecimalMultiple(DecimalMultiple::Kilo))]
    #[case("M", Suffix::DecimalMultiple(DecimalMultiple::Mega))]
    #[case("G", Suffix::DecimalMultiple(DecimalMultiple::Giga))]
    #[case("T", Suffix::DecimalMultiple(DecimalMultiple::Tera))]
    #[case("P", Suffix::DecimalMultiple(DecimalMultiple::Peta))]
    #[case("E", Suffix::DecimalMultiple(DecimalMultiple::Exa))]
    fn decimal_multiple_from_str_pass(#[case] input: &str, #[case] expected: Suffix) {
        let parsed = Suffix::from_str(input).unwrap();
        assert_eq!(parsed, expected);
    }

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
    #[case("1.234e-3.21", Quantity { value: 1.234, suffix: Suffix::DecimalExponent(DecimalExponent(-3.21)) })]
    #[case("1.234E-3.21", Quantity { value: 1.234, suffix: Suffix::DecimalExponent(DecimalExponent(-3.21)) })]
    #[case("1.234e3", Quantity { value: 1.234, suffix: Suffix::DecimalExponent(DecimalExponent(3.0)) })]
    #[case("1.234E3", Quantity { value: 1.234, suffix: Suffix::DecimalExponent(DecimalExponent(3.0)) })]
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
    #[case("1Mi", BinaryMultiple::Kibi, "1024Ki", true)]
    #[case("1024Ki", BinaryMultiple::Mebi, "1Mi", true)]
    #[case("1Mi", BinaryMultiple::Mebi, "1Mi", false)]
    fn binary_to_binary_scale_pass(
        #[case] input: &str,
        #[case] scale_to: BinaryMultiple,
        #[case] output: &str,
        #[case] _scaled: bool,
    ) {
        let parsed = Quantity::from_str(input)
            .unwrap()
            .scale_to(Suffix::BinaryMultiple(scale_to));

        assert_eq!(parsed.to_string(), output);
    }

    #[rstest]
    #[case("1Mi", DecimalMultiple::Kilo, "1048.576k", true)]
    #[case("1Mi", DecimalMultiple::Mega, "1.048576M", true)]
    fn binary_to_decimal_scale_pass(
        #[case] input: &str,
        #[case] scale_to: DecimalMultiple,
        #[case] output: &str,
        #[case] _scaled: bool,
    ) {
        let parsed = Quantity::from_str(input)
            .unwrap()
            .scale_to(Suffix::DecimalMultiple(scale_to));

        assert_eq!(parsed.to_string(), output);
    }

    #[rstest]
    #[case("1M", DecimalMultiple::Kilo, "1000k", true)]
    #[case("1000k", DecimalMultiple::Mega, "1M", true)]
    #[case("1M", DecimalMultiple::Mega, "1M", false)]
    fn decimal_to_decimal_scale_pass(
        #[case] input: &str,
        #[case] scale_to: DecimalMultiple,
        #[case] output: &str,
        #[case] _scaled: bool,
    ) {
        let parsed = Quantity::from_str(input)
            .unwrap()
            .scale_to(Suffix::DecimalMultiple(scale_to));

        assert_eq!(parsed.to_string(), output);
    }

    #[rstest]
    #[case("1e3", DecimalExponent(0.0), "1000e0", true)]
    #[case("1000e0", DecimalExponent(3.0), "1e3", true)]
    #[case("1e3", DecimalExponent(3.0), "1e3", false)]
    fn decimal_exponent_to_decimal_exponent_scale_pass(
        #[case] input: &str,
        #[case] scale_to: DecimalExponent,
        #[case] output: &str,
        #[case] _scaled: bool,
    ) {
        let parsed = Quantity::from_str(input)
            .unwrap()
            .scale_to(Suffix::DecimalExponent(scale_to));

        assert_eq!(parsed.to_string(), output);
    }
}
