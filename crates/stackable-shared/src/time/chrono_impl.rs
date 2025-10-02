use crate::time::Duration;

impl TryFrom<chrono::TimeDelta> for Duration {
    type Error = chrono::OutOfRangeError;

    fn try_from(value: chrono::TimeDelta) -> Result<Self, Self::Error> {
        let std_duration = value.to_std()?;
        Ok(Self::from(std_duration))
    }
}

impl TryFrom<Duration> for chrono::TimeDelta {
    type Error = chrono::OutOfRangeError;

    fn try_from(value: Duration) -> Result<Self, Self::Error> {
        chrono::TimeDelta::from_std(value.into())
    }
}
