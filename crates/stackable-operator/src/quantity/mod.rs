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

    /// The optional suffix of the quantity.
    ///
    /// This field holds data parsed from `<suffix>` according to the spec.
    suffix: Option<Suffix>,
}

impl FromStr for Quantity {
    type Err = ParseQuantityError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        ensure!(!input.is_empty() && input.is_ascii(), InvalidFormatSnafu);

        if input == "0" {
            return Ok(Self {
                value: 0.0,
                suffix: None,
            });
        }

        match input.find(|c: char| c != '.' && !c.is_ascii_digit()) {
            Some(suffix_index) => {
                let parts = input.split_at(suffix_index);
                let value = f64::from_str(parts.0).context(InvalidFloatSnafu)?;
                let suffix = Suffix::from_str(parts.1).context(InvalidSuffixSnafu)?;

                Ok(Self {
                    suffix: Some(suffix),
                    value,
                })
            }
            None => {
                let value = f64::from_str(input).context(InvalidFloatSnafu)?;
                Ok(Self {
                    value,
                    suffix: None,
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

        match &self.suffix {
            Some(suffix) => write!(f, "{value}{suffix}", value = self.value,),
            None => write!(f, "{value}", value = self.value),
        }
    }
}

impl From<Quantity> for K8sQuantity {
    fn from(value: Quantity) -> Self {
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
    // TODO (@Techassi): Discuss if this should consume or mutate in place. Consumption requires us
    // to add these function on specialized quantities (which then forward). If we mutate in place,
    // we could leverage the Deref impl instead.

    /// Optionally scales up or down to the provided `suffix`.
    ///
    /// It additionally returns `true` if the suffix was scaled, and `false` in the following cases:
    ///
    /// - the suffixes already match
    /// - the quantity has no suffix, in which case the suffix will be added without scaling
    /// - the value is 0
    pub fn scale_to(&mut self, suffix: Suffix) -> bool {
        match (&mut self.value, &mut self.suffix) {
            (0.0, _) => false,
            (_, Some(s)) if *s == suffix => false,
            (_, None) => {
                self.suffix = Some(suffix);
                false
            }
            (v, Some(s)) => {
                let factor = (s.base() as f64).powf(s.exponent())
                    / (suffix.base() as f64).powf(suffix.exponent());

                *v *= factor;
                *s = suffix;

                false
            }
        }
    }

    pub fn ceil(&mut self) {
        self.value = self.value.ceil();
    }
}

#[derive(Debug, PartialEq, Snafu)]
#[snafu(display("failed to parse {input:?} as quantity suffix"))]
pub struct ParseSuffixError {
    input: String,
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub enum Suffix {
    DecimalByteMultiple(DecimalByteMultiple),
    BinaryByteMultiple(BinaryByteMultiple),
    DecimalExponent(DecimalExponent),
}

impl FromStr for Suffix {
    type Err = ParseSuffixError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if let Ok(binary_si) = BinaryByteMultiple::from_str(input) {
            return Ok(Self::BinaryByteMultiple(binary_si));
        }

        if let Ok(decimal_si) = DecimalByteMultiple::from_str(input) {
            return Ok(Self::DecimalByteMultiple(decimal_si));
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
            Suffix::DecimalByteMultiple(decimal) => write!(f, "{decimal}"),
            Suffix::BinaryByteMultiple(binary) => write!(f, "{binary}"),
            Suffix::DecimalExponent(float) => write!(f, "e{float}"),
        }
    }
}

impl Suffix {
    pub fn exponent(&self) -> f64 {
        match self {
            Suffix::DecimalByteMultiple(s) => s.exponent(),
            Suffix::BinaryByteMultiple(s) => s.exponent(),
            Suffix::DecimalExponent(s) => s.exponent(),
        }
    }

    pub fn base(&self) -> usize {
        match self {
            Suffix::DecimalByteMultiple(_) => DecimalByteMultiple::BASE,
            Suffix::BinaryByteMultiple(_) => BinaryByteMultiple::BASE,
            Suffix::DecimalExponent(_) => DecimalExponent::BASE,
        }
    }
}

/// Provides a trait for suffix multiples to provide their base and exponent for each unit variant
/// or scientific notation exponent.
pub trait SuffixMultiple {
    /// The base of the multiple.
    const BASE: usize;

    /// Returns the exponent based on the unit variant or scientific notation exponent.
    fn exponent(&self) -> f64;
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
pub enum BinaryByteMultiple {
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

impl SuffixMultiple for BinaryByteMultiple {
    /// The base of the binary byte multiple is 2 because 2^10 = 1024^1 = 1 KiB.
    const BASE: usize = 2;

    fn exponent(&self) -> f64 {
        match self {
            BinaryByteMultiple::Kibi => 10.0,
            BinaryByteMultiple::Mebi => 20.0,
            BinaryByteMultiple::Gibi => 30.0,
            BinaryByteMultiple::Tebi => 40.0,
            BinaryByteMultiple::Pebi => 50.0,
            BinaryByteMultiple::Exbi => 60.0,
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
/// - 1000^ 0,  B ( ): byte      (no suffix, unit-less)
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
pub enum DecimalByteMultiple {
    #[strum(serialize = "m")]
    Milli,

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

impl SuffixMultiple for DecimalByteMultiple {
    const BASE: usize = 10;

    fn exponent(&self) -> f64 {
        match self {
            DecimalByteMultiple::Milli => -3.0,
            DecimalByteMultiple::Kilo => 3.0,
            DecimalByteMultiple::Mega => 6.0,
            DecimalByteMultiple::Giga => 9.0,
            DecimalByteMultiple::Tera => 12.0,
            DecimalByteMultiple::Peta => 15.0,
            DecimalByteMultiple::Exa => 18.0,
        }
    }
}

/// Scientific (also know as E) notation of numbers.
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

impl SuffixMultiple for DecimalExponent {
    const BASE: usize = 10;

    fn exponent(&self) -> f64 {
        self.0
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("Ki", Suffix::BinaryByteMultiple(BinaryByteMultiple::Kibi))]
    #[case("Mi", Suffix::BinaryByteMultiple(BinaryByteMultiple::Mebi))]
    #[case("Gi", Suffix::BinaryByteMultiple(BinaryByteMultiple::Gibi))]
    #[case("Ti", Suffix::BinaryByteMultiple(BinaryByteMultiple::Tebi))]
    #[case("Pi", Suffix::BinaryByteMultiple(BinaryByteMultiple::Pebi))]
    #[case("Ei", Suffix::BinaryByteMultiple(BinaryByteMultiple::Exbi))]
    fn binary_byte_multiple_from_str_pass(#[case] input: &str, #[case] expected: Suffix) {
        let parsed = Suffix::from_str(input).unwrap();
        assert_eq!(parsed, expected);
    }

    #[rstest]
    #[case("m", Suffix::DecimalByteMultiple(DecimalByteMultiple::Milli))]
    #[case("k", Suffix::DecimalByteMultiple(DecimalByteMultiple::Kilo))]
    #[case("M", Suffix::DecimalByteMultiple(DecimalByteMultiple::Mega))]
    #[case("G", Suffix::DecimalByteMultiple(DecimalByteMultiple::Giga))]
    #[case("T", Suffix::DecimalByteMultiple(DecimalByteMultiple::Tera))]
    #[case("P", Suffix::DecimalByteMultiple(DecimalByteMultiple::Peta))]
    #[case("E", Suffix::DecimalByteMultiple(DecimalByteMultiple::Exa))]
    fn decimal_byte_multiple_from_str_pass(#[case] input: &str, #[case] expected: Suffix) {
        let parsed = Suffix::from_str(input).unwrap();
        assert_eq!(parsed, expected);
    }

    #[rstest]
    #[case("49041204Ki", Quantity { value: 49041204.0, suffix: Some(Suffix::BinaryByteMultiple(BinaryByteMultiple::Kibi)) })]
    #[case("256Ki", Quantity { value: 256.0, suffix: Some(Suffix::BinaryByteMultiple(BinaryByteMultiple::Kibi)) })]
    #[case("1.5Gi", Quantity { value: 1.5, suffix: Some(Suffix::BinaryByteMultiple(BinaryByteMultiple::Gibi)) })]
    #[case("0.8Ti", Quantity { value: 0.8, suffix: Some(Suffix::BinaryByteMultiple(BinaryByteMultiple::Tebi)) })]
    #[case("3.2Pi", Quantity { value: 3.2, suffix: Some(Suffix::BinaryByteMultiple(BinaryByteMultiple::Pebi)) })]
    #[case("0.2Ei", Quantity { value: 0.2, suffix: Some(Suffix::BinaryByteMultiple(BinaryByteMultiple::Exbi)) })]
    #[case("8Mi", Quantity { value: 8.0, suffix: Some(Suffix::BinaryByteMultiple(BinaryByteMultiple::Mebi)) })]
    fn binary_quantity_from_str_pass(#[case] input: &str, #[case] expected: Quantity) {
        let parsed = Quantity::from_str(input).unwrap();
        assert_eq!(parsed, expected);
    }

    #[rstest]
    #[case("49041204k", Quantity { value: 49041204.0, suffix: Some(Suffix::DecimalByteMultiple(DecimalByteMultiple::Kilo)) })]
    #[case("256k", Quantity { value: 256.0, suffix: Some(Suffix::DecimalByteMultiple(DecimalByteMultiple::Kilo)) })]
    #[case("1.5G", Quantity { value: 1.5, suffix: Some(Suffix::DecimalByteMultiple(DecimalByteMultiple::Giga)) })]
    #[case("0.8T", Quantity { value: 0.8, suffix: Some(Suffix::DecimalByteMultiple(DecimalByteMultiple::Tera)) })]
    #[case("3.2P", Quantity { value: 3.2, suffix: Some(Suffix::DecimalByteMultiple(DecimalByteMultiple::Peta)) })]
    #[case("0.2E", Quantity { value: 0.2, suffix: Some(Suffix::DecimalByteMultiple(DecimalByteMultiple::Exa)) })]
    #[case("4m", Quantity { value: 4.0, suffix: Some(Suffix::DecimalByteMultiple(DecimalByteMultiple::Milli)) })]
    #[case("8M", Quantity { value: 8.0, suffix: Some(Suffix::DecimalByteMultiple(DecimalByteMultiple::Mega)) })]
    fn decimal_quantity_from_str_pass(#[case] input: &str, #[case] expected: Quantity) {
        let parsed = Quantity::from_str(input).unwrap();
        assert_eq!(parsed, expected);
    }

    #[rstest]
    #[case("1.234e-3.21", Quantity { value: 1.234, suffix: Some(Suffix::DecimalExponent(DecimalExponent(-3.21))) })]
    #[case("1.234E-3.21", Quantity { value: 1.234, suffix: Some(Suffix::DecimalExponent(DecimalExponent(-3.21))) })]
    #[case("1.234e3", Quantity { value: 1.234, suffix: Some(Suffix::DecimalExponent(DecimalExponent(3.0))) })]
    #[case("1.234E3", Quantity { value: 1.234, suffix: Some(Suffix::DecimalExponent(DecimalExponent(3.0))) })]
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
    #[case("1Mi", BinaryByteMultiple::Kibi, "1024Ki", true)]
    #[case("1024Ki", BinaryByteMultiple::Mebi, "1Mi", true)]
    #[case("1Mi", BinaryByteMultiple::Mebi, "1Mi", false)]
    fn binary_byte_to_binary_byte_scale_pass(
        #[case] input: &str,
        #[case] scale_to: BinaryByteMultiple,
        #[case] output: &str,
        #[case] scaled: bool,
    ) {
        let mut parsed = Quantity::from_str(input).unwrap();
        let was_scaled = parsed.scale_to(Suffix::BinaryByteMultiple(scale_to));

        assert_eq!(parsed.to_string(), output);
        assert_eq!(was_scaled, scaled);
    }

    #[rstest]
    #[case("1Mi", DecimalByteMultiple::Kilo, "1048.576k", true)]
    #[case("1Mi", DecimalByteMultiple::Mega, "1.048576M", true)]
    fn binary_byte_to_decimal_byte_scale_pass(
        #[case] input: &str,
        #[case] scale_to: DecimalByteMultiple,
        #[case] output: &str,
        #[case] scaled: bool,
    ) {
        let mut parsed = Quantity::from_str(input).unwrap();
        let was_scaled = parsed.scale_to(Suffix::DecimalByteMultiple(scale_to));

        assert_eq!(parsed.to_string(), output);
        assert_eq!(was_scaled, scaled);
    }

    #[rstest]
    #[case("1M", DecimalByteMultiple::Kilo, "1000k", true)]
    #[case("1000k", DecimalByteMultiple::Mega, "1M", true)]
    #[case("1M", DecimalByteMultiple::Mega, "1M", false)]
    fn decimal_byte_to_decimal_byte_scale_pass(
        #[case] input: &str,
        #[case] scale_to: DecimalByteMultiple,
        #[case] output: &str,
        #[case] scaled: bool,
    ) {
        let mut parsed = Quantity::from_str(input).unwrap();
        let was_scaled = parsed.scale_to(Suffix::DecimalByteMultiple(scale_to));

        assert_eq!(parsed.to_string(), output);
        assert_eq!(was_scaled, scaled);
    }

    #[rstest]
    #[case("1e3", DecimalExponent(0.0), "1000e0", true)]
    #[case("1000e0", DecimalExponent(3.0), "1e3", true)]
    #[case("1e3", DecimalExponent(3.0), "1e3", false)]
    fn decimal_exponent_to_decimal_exponent_scale_pass(
        #[case] input: &str,
        #[case] scale_to: DecimalExponent,
        #[case] output: &str,
        #[case] scaled: bool,
    ) {
        let mut parsed = Quantity::from_str(input).unwrap();
        let was_scaled = parsed.scale_to(Suffix::DecimalExponent(scale_to));

        assert_eq!(parsed.to_string(), output);
        assert_eq!(was_scaled, scaled);
    }
}
