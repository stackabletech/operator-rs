use jiff::Span;

use crate::time::Duration;

impl TryFrom<Span> for Duration {
    type Error = jiff::Error;

    fn try_from(value: Span) -> Result<Self, Self::Error> {
        let std_duration = std::time::Duration::try_from(value)?;
        Ok(Self::from(std_duration))
    }
}

impl TryFrom<Duration> for Span {
    type Error = jiff::Error;

    fn try_from(value: Duration) -> Result<Self, Self::Error> {
        Span::try_from(Into::<std::time::Duration>::into(value))
    }
}
