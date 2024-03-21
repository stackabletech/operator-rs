//! Extensions for use cases that [`Iterator`] doesn't handle natively quite yet

/// Fallible version of [`Iterator::flatten`]
///
/// If the outer [`Iterator`] returns [`Ok`] then each item in the inner iterator is emitted,
/// otherwise the outer [`Err`] is passed through.
pub fn try_flatten<I1, I2, T, E>(outer_iterator: I1) -> TryFlatten<I1::IntoIter, I2::IntoIter>
where
    I1: IntoIterator<Item = Result<I2, E>>,
    I2: IntoIterator<Item = Result<T, E>>,
{
    TryFlatten {
        outer_iterator: outer_iterator.into_iter(),
        inner_iterator: None,
    }
}

/// See [`try_flatten`]
pub struct TryFlatten<I1, I2> {
    outer_iterator: I1,
    inner_iterator: Option<I2>,
}

impl<I1, I2, T, E> Iterator for TryFlatten<I1, I2::IntoIter>
where
    I1: Iterator<Item = Result<I2, E>>,
    I2: IntoIterator<Item = Result<T, E>>,
{
    type Item = Result<T, E>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(inner) = &mut self.inner_iterator {
                match inner.next() {
                    Some(value) => return Some(value),
                    None => self.inner_iterator = None,
                }
            }

            match self.outer_iterator.next()? {
                Ok(inner) => self.inner_iterator = Some(inner.into_iter()),
                Err(err) => return Some(Err(err)),
            }
        }
    }
}

/// This is a fallible version of the std [`FromIterator`] trait.
///
/// The standard [`FromIterator`] trait specifies it must never fail. This trait
/// makes it easier to work with iterators, which can fail during the creation
/// `Self`. It will immediately return an error if processing failed and will
/// not continue to process items.
pub trait TryFromIterator<T>: Sized {
    type Error: std::error::Error;

    fn try_from_iter<I: IntoIterator<Item = T>>(iter: I) -> Result<Self, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_flatten_marble_test() {
        let stream = vec![
            Ok(vec![Ok(1), Err(2), Ok(3)]),
            Err(4),
            Err(5),
            Ok(vec![Ok(6)]),
        ];
        assert_eq!(
            try_flatten(stream).collect::<Vec<_>>(),
            vec![Ok(1), Err(2), Ok(3), Err(4), Err(5), Ok(6)],
        );
    }

    #[test]
    fn try_from_iter_success() {
        let iter = [1, 2, 3, 4];

        #[derive(Debug, PartialEq)]
        struct Sum(usize);

        impl TryFromIterator<usize> for Sum {
            type Error = std::convert::Infallible;

            fn try_from_iter<I: IntoIterator<Item = usize>>(iter: I) -> Result<Self, Self::Error> {
                let sum = iter.into_iter().sum();
                Ok(Sum(sum))
            }
        }

        assert_eq!(Sum(10), Sum::try_from_iter(iter).unwrap());
    }

    #[test]
    fn try_from_iter_error() {
        let iter = ["1", "2", "3", "-4"];

        #[derive(Debug, PartialEq)]
        struct Sum(usize);

        impl<T> TryFromIterator<T> for Sum
        where
            T: AsRef<str>,
        {
            type Error = std::num::ParseIntError;

            fn try_from_iter<I: IntoIterator<Item = T>>(iter: I) -> Result<Self, Self::Error> {
                let mut sum = 0;

                for item in iter {
                    sum += item.as_ref().parse::<usize>()?;
                }

                Ok(Self(sum))
            }
        }

        assert!(Sum::try_from_iter(iter).is_err());
    }
}
