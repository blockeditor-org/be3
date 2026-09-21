use std::ops::{Add, AddAssign, Mul, Sub};

#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self::new(0.0, 0.0);
    pub const INFINITY: Self = Self::new(f32::INFINITY, f32::INFINITY);

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub const fn splat(value: f32) -> Self {
        Self::new(value, value)
    }

    pub fn max(self, other: Self) -> Self {
        Self::new(self.x.max(other.x), self.y.max(other.y))
    }

    pub fn min(self, other: Self) -> Self {
        Self::new(self.x.min(other.x), self.y.min(other.y))
    }

    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    pub fn longest_side(self) -> f32 {
        self.x.max(self.y)
    }

    pub fn shortest_side(self) -> f32 {
        self.x.min(self.y)
    }
}

pub const fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

impl Add for Vec2 {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for Vec2 {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Self;

    fn mul(self, factor: f32) -> Self {
        Self::new(self.x * factor, self.y * factor)
    }
}

#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub struct Pos2 {
    pub x: f32,
    pub y: f32,
}

impl Pos2 {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub const fn to_vec2(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }

    pub fn distance(self, other: Self) -> f32 {
        (self.x - other.x).hypot(self.y - other.y)
    }
}

pub const fn pos2(x: f32, y: f32) -> Pos2 {
    Pos2::new(x, y)
}

impl Add<Vec2> for Pos2 {
    type Output = Self;

    fn add(self, offset: Vec2) -> Self {
        Self::new(self.x + offset.x, self.y + offset.y)
    }
}

impl AddAssign<Vec2> for Pos2 {
    fn add_assign(&mut self, offset: Vec2) {
        self.x += offset.x;
        self.y += offset.y;
    }
}

impl Sub<Vec2> for Pos2 {
    type Output = Self;

    fn sub(self, offset: Vec2) -> Self {
        Self::new(self.x - offset.x, self.y - offset.y)
    }
}

impl Sub for Pos2 {
    type Output = Vec2;

    fn sub(self, other: Self) -> Vec2 {
        Vec2::new(self.x - other.x, self.y - other.y)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rect {
    pub min: Pos2,
    pub max: Pos2,
}

impl Rect {
    pub const ZERO: Self = Self {
        min: Pos2::ZERO,
        max: Pos2::ZERO,
    };

    pub const NOTHING: Self = Self {
        min: Pos2::new(f32::INFINITY, f32::INFINITY),
        max: Pos2::new(f32::NEG_INFINITY, f32::NEG_INFINITY),
    };

    pub const EVERYTHING: Self = Self {
        min: Pos2::new(f32::NEG_INFINITY, f32::NEG_INFINITY),
        max: Pos2::new(f32::INFINITY, f32::INFINITY),
    };

    pub const fn from_min_max(min: Pos2, max: Pos2) -> Self {
        Self { min, max }
    }

    pub fn from_min_size(min: Pos2, size: Vec2) -> Self {
        Self {
            min,
            max: min + size,
        }
    }

    pub fn left(&self) -> f32 {
        self.min.x
    }

    pub fn right(&self) -> f32 {
        self.max.x
    }

    pub fn top(&self) -> f32 {
        self.min.y
    }

    pub fn bottom(&self) -> f32 {
        self.max.y
    }

    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }

    pub fn center(&self) -> Pos2 {
        Pos2::new(
            (self.min.x + self.max.x) / 2.0,
            (self.min.y + self.max.y) / 2.0,
        )
    }

    pub fn contains(&self, point: Pos2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    pub fn contains_rect(&self, other: Self) -> bool {
        self.min.x <= other.min.x
            && self.min.y <= other.min.y
            && self.max.x >= other.max.x
            && self.max.y >= other.max.y
    }

    pub fn expand(&self, amount: f32) -> Self {
        Self {
            min: self.min - Vec2::splat(amount),
            max: self.max + Vec2::splat(amount),
        }
    }

    pub fn shrink(&self, amount: f32) -> Self {
        self.expand(-amount)
    }

    pub fn union(&self, other: Self) -> Self {
        Self {
            min: Pos2::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: Pos2::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }

    pub fn intersect(&self, other: Self) -> Self {
        Self {
            min: Pos2::new(self.min.x.max(other.min.x), self.min.y.max(other.min.y)),
            max: Pos2::new(self.max.x.min(other.max.x), self.max.y.min(other.max.y)),
        }
    }

    pub(crate) fn scaled(&self, factor: f32) -> Self {
        Self::from_min_max(
            pos2(self.min.x * factor, self.min.y * factor),
            pos2(self.max.x * factor, self.max.y * factor),
        )
    }

    pub fn translate(&self, offset: Vec2) -> Self {
        Self {
            min: self.min + offset,
            max: self.max + offset,
        }
    }

    pub fn intersects(&self, other: Self) -> bool {
        self.min.x < other.max.x
            && other.min.x < self.max.x
            && self.min.y < other.max.y
            && other.min.y < self.max.y
    }

    pub fn is_positive(&self) -> bool {
        self.min.x < self.max.x && self.min.y < self.max.y
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rotation {
    pub pivot: Pos2,
    pub angle: f32,
}

impl Rotation {
    pub const NONE: Self = Self {
        pivot: Pos2::ZERO,
        angle: 0.0,
    };

    pub const fn new(pivot: Pos2, angle: f32) -> Self {
        Self { pivot, angle }
    }

    pub fn turns(self) -> bool {
        self.angle != 0.0
    }

    pub fn apply(self, point: Pos2) -> Pos2 {
        self.turn(point, self.angle)
    }

    pub fn undo(self, point: Pos2) -> Pos2 {
        self.turn(point, -self.angle)
    }

    pub fn bounds(self, rect: Rect) -> Rect {
        if !self.turns() {
            return rect;
        }
        let corners = [
            rect.min,
            pos2(rect.max.x, rect.min.y),
            rect.max,
            pos2(rect.min.x, rect.max.y),
        ]
        .map(|corner| self.apply(corner));
        let mut bounds = Rect::from_min_max(corners[0], corners[0]);
        for corner in corners {
            bounds = bounds.union(Rect::from_min_max(corner, corner));
        }
        bounds
    }

    pub fn scaled(self, scale: f32) -> Self {
        Self {
            pivot: pos2(self.pivot.x * scale, self.pivot.y * scale),
            angle: self.angle,
        }
    }

    fn turn(self, point: Pos2, angle: f32) -> Pos2 {
        if angle == 0.0 {
            return point;
        }
        let (sin, cos) = angle.sin_cos();
        let x = point.x - self.pivot.x;
        let y = point.y - self.pivot.y;
        pos2(
            self.pivot.x + x * cos - y * sin,
            self.pivot.y + x * sin + y * cos,
        )
    }
}
