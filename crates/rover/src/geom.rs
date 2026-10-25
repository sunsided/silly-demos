//! Points and angles.

use std::ops::{Add, Mul, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// The unit vector at `angle` (clockwise from +x).
    pub fn from_angle(angle: f32) -> Self {
        let (sin, cos) = angle.sin_cos();
        Self::new(cos, sin)
    }

    pub fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn distance(self, o: Self) -> f32 {
        (self - o).length()
    }

    /// Direction of the vector, clockwise from +x.
    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }
}

impl Add for Point {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Point {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for Point {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s)
    }
}

/// Position and heading of the robot.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose {
    pub x: f32,
    pub y: f32,
    /// Heading, clockwise from +x.
    pub theta: f32,
}

impl Pose {
    pub const fn new(x: f32, y: f32, theta: f32) -> Self {
        Self { x, y, theta }
    }

    pub const fn position(self) -> Point {
        Point::new(self.x, self.y)
    }
}

/// `a` wrapped into (−π, π].
pub fn wrap(a: f32) -> f32 {
    a.sin().atan2(a.cos())
}

/// Distance from `p` to the segment `a`–`b`.
pub fn segment_distance(p: Point, a: Point, b: Point) -> f32 {
    let e = b - a;
    let ee = e.dot(e);
    let t = if ee > 0.0 {
        ((p - a).dot(e) / ee).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(a + e * t)
}
