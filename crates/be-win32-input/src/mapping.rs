use beui_core::geometry::{Pos2, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct PixelPoint {
    pub x: i32,
    pub y: i32,
}

impl PixelPoint {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct PixelRect {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

impl PixelRect {
    pub const fn new(left: i32, top: i32, width: i32, height: i32) -> Self {
        Self {
            left,
            top,
            width,
            height,
        }
    }

    pub const fn from_edges(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self::new(left, top, right - left, bottom - top)
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    pub fn contains(&self, point: PixelPoint) -> bool {
        (self.left..self.left + self.width).contains(&point.x)
            && (self.top..self.top + self.height).contains(&point.y)
    }

    pub fn clamp(&self, point: PixelPoint) -> PixelPoint {
        PixelPoint::new(
            point
                .x
                .clamp(self.left, self.left + (self.width - 1).max(0)),
            point.y.clamp(self.top, self.top + (self.height - 1).max(0)),
        )
    }

    pub fn point_at(&self, size: Vec2, at: Pos2) -> PixelPoint {
        let along = |at: f32, size: f32, start: i32, length: i32| {
            let fraction = match size > 0.0 {
                true => at / size,
                false => 0.0,
            };
            start + (fraction * length as f32).floor() as i32
        };
        self.clamp(PixelPoint::new(
            along(at.x, size.x, self.left, self.width),
            along(at.y, size.y, self.top, self.height),
        ))
    }
}

pub const ABSOLUTE_MAX: i32 = 65535;

pub fn absolute(desktop: PixelRect, point: PixelPoint) -> PixelPoint {
    let point = desktop.clamp(point);
    let along = |offset: i32, length: i32| {
        let length = i64::from(length.max(1));
        let centred = (2 * i64::from(offset) + 1) * 65536 / (2 * length);
        centred.clamp(0, i64::from(ABSOLUTE_MAX)) as i32
    };
    PixelPoint::new(
        along(point.x - desktop.left, desktop.width),
        along(point.y - desktop.top, desktop.height),
    )
}

pub fn from_absolute(desktop: PixelRect, absolute: PixelPoint) -> PixelPoint {
    let along = |value: i32, length: i32| (i64::from(value) * i64::from(length) / 65536) as i32;
    PixelPoint::new(
        desktop.left + along(absolute.x, desktop.width),
        desktop.top + along(absolute.y, desktop.height),
    )
}

#[cfg(test)]
mod tests;
