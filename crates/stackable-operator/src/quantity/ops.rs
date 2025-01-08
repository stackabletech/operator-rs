use std::ops::{Add, AddAssign, Div, Mul, MulAssign, Sub, SubAssign};

use crate::quantity::Quantity;

impl Add for Quantity {
    type Output = Quantity;

    fn add(self, rhs: Quantity) -> Self::Output {
        let rhs = rhs.scale_to(self.suffix);

        Self {
            value: self.value + rhs.value,
            ..self
        }
    }
}

impl AddAssign for Quantity {
    fn add_assign(&mut self, rhs: Quantity) {
        *self = self.add(rhs)
    }
}

impl Sub for Quantity {
    type Output = Quantity;

    fn sub(self, rhs: Quantity) -> Self::Output {
        let rhs = rhs.scale_to(self.suffix);

        Self {
            value: self.value - rhs.value,
            ..self
        }
    }
}

impl SubAssign for Quantity {
    fn sub_assign(&mut self, rhs: Self) {
        *self = self.sub(rhs)
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
        *self = self.mul(rhs)
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
        *self = self.mul(rhs)
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
        *self = self.mul(rhs)
    }
}

impl Div for Quantity {
    type Output = f64;

    fn div(self, rhs: Self) -> Self::Output {
        let rhs = rhs.scale_to(self.suffix);
        self.value / rhs.value
    }
}
