use std::ops::{Add, Sub, Mul, Div, AddAssign, SubAssign, MulAssign, DivAssign};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector2D {
    pub x: f64,
    pub y: f64,
}

impl Vector2D {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn floor(self) -> Self {
        Self {
            x: self.x.floor(),
            y: self.y.floor(),
        }
    }

    pub fn round(self) -> Self {
        Self {
            x: self.x.round(),
            y: self.y.round(),
        }
    }

    pub fn ceil(self) -> Self {
        Self {
            x: self.x.ceil(),
            y: self.y.ceil(),
        }
    }

    pub fn length(self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn normalize(self) -> Self {
        let len = self.length();
        if len == 0.0 {
            self
        } else {
            self / len
        }
    }
}

impl Default for Vector2D {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

// Addition
impl Add for Vector2D {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl AddAssign for Vector2D {
    fn add_assign(&mut self, other: Self) {
        self.x += other.x;
        self.y += other.y;
    }
}

// Subtraction
impl Sub for Vector2D {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
}

impl SubAssign for Vector2D {
    fn sub_assign(&mut self, other: Self) {
        self.x -= other.x;
        self.y -= other.y;
    }
}

// Multiplication (scalar and component-wise)
impl Mul<f64> for Vector2D {
    type Output = Self;

    fn mul(self, scalar: f64) -> Self {
        Self {
            x: self.x * scalar,
            y: self.y * scalar,
        }
    }
}

impl Mul<Vector2D> for Vector2D {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        Self {
            x: self.x * other.x,
            y: self.y * other.y,
        }
    }
}

impl MulAssign<f64> for Vector2D {
    fn mul_assign(&mut self, scalar: f64) {
        self.x *= scalar;
        self.y *= scalar;
    }
}

impl MulAssign<Vector2D> for Vector2D {
    fn mul_assign(&mut self, other: Self) {
        self.x *= other.x;
        self.y *= other.y;
    }
}

// Division (scalar and component-wise)
impl Div<f64> for Vector2D {
    type Output = Self;

    fn div(self, scalar: f64) -> Self {
        Self {
            x: self.x / scalar,
            y: self.y / scalar,
        }
    }
}

impl Div<Vector2D> for Vector2D {
    type Output = Self;

    fn div(self, other: Self) -> Self {
        Self {
            x: self.x / other.x,
            y: self.y / other.y,
        }
    }
}

impl DivAssign<f64> for Vector2D {
    fn div_assign(&mut self, scalar: f64) {
        self.x /= scalar;
        self.y /= scalar;
    }
}

impl DivAssign<Vector2D> for Vector2D {
    fn div_assign(&mut self, other: Self) {
        self.x /= other.x;
        self.y /= other.y;
    }
}

// Conversion from (f64, f64)
impl From<(f64, f64)> for Vector2D {
    fn from((x, y): (f64, f64)) -> Self {
        Self { x, y }
    }
}

// Conversion from (i32, i32)
impl From<(i32, i32)> for Vector2D {
    fn from((x, y): (i32, i32)) -> Self {
        Self {
            x: x as f64,
            y: y as f64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_operations() {
        let v1 = Vector2D::new(3.0, 4.0);
        let v2 = Vector2D::new(1.0, 2.0);

        assert_eq!(v1 + v2, Vector2D::new(4.0, 6.0));
        assert_eq!(v1 - v2, Vector2D::new(2.0, 2.0));
        assert_eq!(v1 * 2.0, Vector2D::new(6.0, 8.0));
        assert_eq!(v1 / 2.0, Vector2D::new(1.5, 2.0));
    }

    #[test]
    fn test_floor_round_ceil() {
        let v = Vector2D::new(3.7, 4.2);
        assert_eq!(v.floor(), Vector2D::new(3.0, 4.0));
        assert_eq!(v.round(), Vector2D::new(4.0, 4.0));
        assert_eq!(v.ceil(), Vector2D::new(4.0, 5.0));
    }

    #[test]
    fn test_length() {
        let v = Vector2D::new(3.0, 4.0);
        assert_eq!(v.length(), 5.0);
    }
}
