use crate::geometry::{Pos2, Rect, Vec2, pos2, vec2};

const TOLERANCE: f32 = 1.0 / 1024.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct PixelGrid {
    scale: f32,
}

impl Default for PixelGrid {
    fn default() -> Self {
        Self { scale: 1.0 }
    }
}

impl PixelGrid {
    pub(crate) fn new(scale: f32) -> Self {
        match scale.is_finite() && scale > 0.0 {
            true => Self { scale },
            false => Self::default(),
        }
    }

    pub(crate) fn snap(self, value: f32) -> f32 {
        match value.is_finite() {
            true => (value * self.scale).round() / self.scale,
            false => value,
        }
    }

    pub(crate) fn snap_up(self, value: f32) -> f32 {
        if !value.is_finite() {
            return value;
        }
        let pixels = value * self.scale;
        let nearest = pixels.round();
        let snapped = match (pixels - nearest).abs() <= TOLERANCE {
            true => nearest,
            false => pixels.ceil(),
        };
        snapped / self.scale
    }

    pub(crate) fn snap_vec(self, size: Vec2) -> Vec2 {
        vec2(self.snap(size.x), self.snap(size.y))
    }

    pub(crate) fn snap_size(self, size: Vec2) -> Vec2 {
        vec2(self.snap_up(size.x), self.snap_up(size.y))
    }

    pub(crate) fn snap_pos(self, point: Pos2) -> Pos2 {
        pos2(self.snap(point.x), self.snap(point.y))
    }

    pub(crate) fn snap_rect(self, rect: Rect) -> Rect {
        Rect::from_min_max(self.snap_pos(rect.min), self.snap_pos(rect.max))
    }
}

#[cfg(test)]
mod tests;
