use std::ops::{Add, AddAssign, Sub, SubAssign};

use crate::time::Duration;

impl Add<Duration> for time::OffsetDateTime {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        self.add(*rhs)
    }
}

impl AddAssign<Duration> for time::OffsetDateTime {
    fn add_assign(&mut self, rhs: Duration) {
        self.add_assign(*rhs)
    }
}

impl Sub<Duration> for time::OffsetDateTime {
    type Output = Self;

    fn sub(self, rhs: Duration) -> Self::Output {
        self.sub(*rhs)
    }
}

impl SubAssign<Duration> for time::OffsetDateTime {
    fn sub_assign(&mut self, rhs: Duration) {
        self.sub_assign(*rhs)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn add_ops() {
        let now = time::OffsetDateTime::now_utc();
        let later = now + Duration::from_minutes_unchecked(10);

        assert!(now < later);
        assert_eq!(
            later.unix_timestamp() - now.unix_timestamp(),
            Duration::from_minutes_unchecked(10).as_secs() as i64
        );
    }

    #[test]
    fn sub_ops() {
        let now = time::OffsetDateTime::now_utc();
        let earlier = now - Duration::from_minutes_unchecked(10);

        assert!(now > earlier);
        assert_eq!(
            now.unix_timestamp() - earlier.unix_timestamp(),
            Duration::from_minutes_unchecked(10).as_secs() as i64
        );
    }
}
