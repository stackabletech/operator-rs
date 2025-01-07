/// This macro is intended to be used to implement the From and TryFrom traits on specialized
/// quantities.
///
/// Currently two specialized quantities exist: [`MemoryQuantity`][1] and [`CpuQuantity`][2].
/// The traits are implemented by forwarding to the inner [`Quantity`][3] implementation. Both
/// specialized quantities are just newtypes / wrappers around [`Quantity`][3].
///
/// [1]: super::MemoryQuantity
/// [2]: super::CpuQuantity
/// [3]: super::Quantity
macro_rules! forward_from_impls {
    ($q:ty, $kq:ty, $for:ty) => {
        impl From<$q> for $for {
            fn from(quantity: $q) -> Self {
                Self(quantity)
            }
        }

        impl TryFrom<$kq> for $for {
            type Error = ParseQuantityError;

            fn try_from(value: $kq) -> Result<Self, Self::Error> {
                Ok(Self(Quantity::try_from(value)?))
            }
        }

        impl TryFrom<&$kq> for $for {
            type Error = ParseQuantityError;

            fn try_from(value: &$kq) -> Result<Self, Self::Error> {
                Ok(Self(Quantity::try_from(value)?))
            }
        }
    };
}

macro_rules! forward_op_impls {
    ($acc:expr, $for:ty, $($on:ty),+) => {
        impl ::std::ops::Add for $for {
            type Output = $for;

            fn add(self, rhs: $for) -> Self::Output {
                Self(self.0 + rhs.0)
            }
        }

        impl ::std::ops::AddAssign for $for {
            fn add_assign(&mut self, rhs: $for) {
                self.0 += rhs.0
            }
        }

        impl ::std::ops::Sub for $for {
            type Output = $for;

            fn sub(self, rhs: $for) -> Self::Output {
                Self(self.0 - rhs.0)
            }
        }

        impl ::std::ops::SubAssign for $for {
            fn sub_assign(&mut self, rhs: $for) {
                self.0 -= rhs.0
            }
        }

        impl ::std::ops::Div for $for {
            type Output = f64;

            fn div(self, rhs: $for) -> Self::Output {
                self.0 / rhs.0
            }
        }

        impl ::std::iter::Sum for $for {
            fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
                iter.fold(
                    $acc,
                    <$for as ::std::ops::Add>::add,
                )
            }
        }

        $(
            impl ::std::ops::Mul<$on> for $for {
                type Output = $for;

                fn mul(self, rhs: $on) -> Self::Output {
                    Self(self.0 * rhs)
                }
            }

            impl ::std::ops::MulAssign<$on> for $for {
                fn mul_assign(&mut self, rhs: $on) {
                    self.0 *= rhs
                }
            }
        )*
    };
}

/// HACK: Make the macros only available in this crate.
pub(crate) use forward_from_impls;
pub(crate) use forward_op_impls;
