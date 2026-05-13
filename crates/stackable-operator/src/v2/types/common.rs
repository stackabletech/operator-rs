//! Common types that do not belong (yet) to a more specific module
use snafu::{ResultExt, Snafu};
use strum::{EnumDiscriminants, IntoStaticStr};

#[derive(Snafu, Debug, EnumDiscriminants)]
#[strum_discriminants(derive(IntoStaticStr))]
pub enum Error {
    #[snafu(display("failed to convert to port number"))]
    ConvertToPortNumber { source: std::num::TryFromIntError },
}

/// A port number
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Port(pub u16);

impl std::fmt::Display for Port {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<u16> for Port {
    fn from(value: u16) -> Self {
        Port(value)
    }
}

impl From<Port> for i32 {
    fn from(value: Port) -> Self {
        value.0 as i32
    }
}

impl TryFrom<i32> for Port {
    type Error = Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Ok(Port(
            u16::try_from(value).context(ConvertToPortNumberSnafu)?,
        ))
    }
}

#[cfg(test)]
mod tests {

    use super::{ErrorDiscriminants, Port};

    #[test]
    fn test_port_fmt() {
        assert_eq!("0".to_owned(), Port(0).to_string());
        assert_eq!("65535".to_owned(), Port(65535).to_string());
    }

    #[test]
    fn test_port_try_from_i32() {
        assert_eq!(Some(Port(0)), Port::try_from(0).ok());
        assert_eq!(Some(Port(65535)), Port::try_from(65535).ok());
        assert_eq!(
            Err(ErrorDiscriminants::ConvertToPortNumber),
            Port::try_from(-1).map_err(ErrorDiscriminants::from)
        );
        assert_eq!(
            Err(ErrorDiscriminants::ConvertToPortNumber),
            Port::try_from(65536).map_err(ErrorDiscriminants::from)
        );
    }
}
