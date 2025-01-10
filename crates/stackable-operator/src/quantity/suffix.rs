use std::{fmt::Display, ops::Deref, str::FromStr};

pub trait Suffix:
    FromStr<Err: std::error::Error + 'static>
    + Clone
    + Copy
    + Default
    + Display
    + PartialEq
    + PartialOrd
{
    fn factor(&self) -> f64;
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

    pub fn scale_down(self) -> Option<Self> {
        match self {
            BinaryMultiple::Kibi => None,
            BinaryMultiple::Mebi => Some(BinaryMultiple::Kibi),
            BinaryMultiple::Gibi => Some(BinaryMultiple::Mebi),
            BinaryMultiple::Tebi => Some(BinaryMultiple::Gibi),
            BinaryMultiple::Pebi => Some(BinaryMultiple::Tebi),
            BinaryMultiple::Exbi => Some(BinaryMultiple::Pebi),
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
    #[strum(serialize = "n")]
    Nano,

    #[strum(serialize = "u")]
    Micro,

    #[strum(serialize = "m")]
    Milli,

    #[strum(serialize = "")]
    Empty,

    // (Note that 1024 = 1Ki but 1000 = 1k; I didn't choose the capitalization.)
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
            DecimalMultiple::Nano => 10f64.powi(-9),
            DecimalMultiple::Micro => 10f64.powi(-6),
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

impl From<f64> for DecimalExponent {
    fn from(value: f64) -> Self {
        Self(value)
    }
}

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
    #[case("Ki", BinaryMultiple::Kibi)]
    #[case("Mi", BinaryMultiple::Mebi)]
    #[case("Gi", BinaryMultiple::Gibi)]
    #[case("Ti", BinaryMultiple::Tebi)]
    #[case("Pi", BinaryMultiple::Pebi)]
    #[case("Ei", BinaryMultiple::Exbi)]
    fn binary_multiple_from_str_pass(#[case] input: &str, #[case] expected: BinaryMultiple) {
        let parsed = BinaryMultiple::from_str(input).unwrap();
        assert_eq!(parsed, expected);
    }

    #[rstest]
    #[case("n", DecimalMultiple::Nano)]
    #[case("u", DecimalMultiple::Micro)]
    #[case("m", DecimalMultiple::Milli)]
    #[case("", DecimalMultiple::Empty)]
    #[case("k", DecimalMultiple::Kilo)]
    #[case("M", DecimalMultiple::Mega)]
    #[case("G", DecimalMultiple::Giga)]
    #[case("T", DecimalMultiple::Tera)]
    #[case("P", DecimalMultiple::Peta)]
    #[case("E", DecimalMultiple::Exa)]
    fn decimal_multiple_from_str_pass(#[case] input: &str, #[case] expected: DecimalMultiple) {
        let parsed = DecimalMultiple::from_str(input).unwrap();
        assert_eq!(parsed, expected);
    }
}
