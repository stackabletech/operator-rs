use std::{
    iter::Sum,
    ops::{Add, AddAssign, Div, Mul, MulAssign, Sub, SubAssign},
};

use crate::quantity::Quantity;

impl Add for Quantity {
    type Output = Quantity;

    fn add(self, rhs: Quantity) -> Self::Output {
        Self {
            value: self.value + rhs.value,
            ..self
        }
    }
}

impl AddAssign for Quantity {
    fn add_assign(&mut self, rhs: Quantity) {
        self.value += rhs.value
    }
}

impl Sub for Quantity {
    type Output = Quantity;

    fn sub(self, rhs: Quantity) -> Self::Output {
        Self {
            value: self.value - rhs.value,
            ..self
        }
    }
}

impl SubAssign for Quantity {
    fn sub_assign(&mut self, rhs: Self) {
        self.value -= rhs.value
    }
}

impl Mul<usize> for Quantity {
    type Output = Quantity;

    fn mul(self, rhs: usize) -> Self::Output {
        Self {
            value: self.value * rhs as f64,
            ..self
        }
    }
}

impl MulAssign<usize> for Quantity {
    fn mul_assign(&mut self, rhs: usize) {
        self.value *= rhs as f64
    }
}

impl Mul<f32> for Quantity {
    type Output = Quantity;

    fn mul(self, rhs: f32) -> Self::Output {
        Self {
            value: self.value * rhs as f64,
            ..self
        }
    }
}

impl MulAssign<f32> for Quantity {
    fn mul_assign(&mut self, rhs: f32) {
        self.value *= rhs as f64
    }
}

impl Mul<f64> for Quantity {
    type Output = Quantity;

    fn mul(self, rhs: f64) -> Self::Output {
        Self {
            value: self.value * rhs,
            ..self
        }
    }
}

impl MulAssign<f64> for Quantity {
    fn mul_assign(&mut self, rhs: f64) {
        self.value *= rhs
    }
}

impl Div for Quantity {
    type Output = f64;

    fn div(self, rhs: Self) -> Self::Output {
        self.value / rhs.value
    }
}

impl Sum for Quantity {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(
            Quantity {
                value: 0.0,
                suffix: None,
            },
            Quantity::add,
        )
    }
}
