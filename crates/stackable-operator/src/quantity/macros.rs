macro_rules! forward_quantity_impls {
    ($for:ty, $kq:ty, $($on:ty),+) => {
        $crate::quantity::macros::forward_from_impls!($for, $kq);
        $crate::quantity::macros::forward_op_impls!($for, $($on),*);
    };
}

macro_rules! forward_from_impls {
    ($for:ty, $kq:ty) => {
        impl ::std::str::FromStr for $for {
            type Err = $crate::quantity::ParseQuantityError;

            fn from_str(input: &str) -> Result<Self, Self::Err> {
                let quantity = $crate::quantity::Quantity::from_str(input)?;
                Ok(Self(quantity))
            }
        }

        impl From<$crate::quantity::Quantity> for $for {
            fn from(quantity: $crate::quantity::Quantity) -> Self {
                Self(quantity)
            }
        }

        impl TryFrom<$kq> for $for {
            type Error = $crate::quantity::ParseQuantityError;

            fn try_from(value: $kq) -> Result<Self, Self::Error> {
                Ok(Self($crate::quantity::Quantity::try_from(value)?))
            }
        }

        impl TryFrom<&$kq> for $for {
            type Error = $crate::quantity::ParseQuantityError;

            fn try_from(value: &$kq) -> Result<Self, Self::Error> {
                Ok(Self(Quantity::try_from(value)?))
            }
        }
    };
}

macro_rules! forward_op_impls {
    ($for:ty, $($on:ty),+) => {
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

// HACK: Make the macros only available in this crate.
pub(crate) use forward_from_impls;
pub(crate) use forward_op_impls;
pub(crate) use forward_quantity_impls;
