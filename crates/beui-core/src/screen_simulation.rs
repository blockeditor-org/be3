use crate::color::Color32;
use crate::geometry::{Pos2, Rect, Vec2, pos2, vec2};

pub const MARGIN: f32 = 20.0;
pub const MINIMUM_SIZE: f32 = 100.0;
pub const MAXIMUM_SIZE: f32 = 8000.0;
pub const BACKDROP: Color32 = Color32::from_rgb(8, 10, 14);

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ScreenSimulation {
    pub size: Vec2,
    pub zoom: Option<f32>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Placement {
    pub scale: f32,
    pub screen: Rect,
}

impl Placement {
    pub fn shown(self) -> Rect {
        self.screen.scaled(self.scale)
    }
}

impl ScreenSimulation {
    pub fn resized(self, size: Vec2) -> Self {
        Self {
            size: vec2(clamp_size(size.x), clamp_size(size.y)),
            ..self
        }
    }

    pub fn rotated(self) -> Self {
        self.resized(vec2(self.size.y, self.size.x))
    }

    pub fn place(
        self,
        stage: Rect,
        pointer: Option<Pos2>,
        pixels_per_point: f32,
    ) -> Option<Placement> {
        let size = self.size;
        if !(size.x.is_finite() && size.y.is_finite() && size.x > 0.0 && size.y > 0.0) {
            return None;
        }
        let area = stage.shrink(MARGIN);
        if !area.is_positive() {
            return None;
        }
        let fit = (area.width() / size.x).min(area.height() / size.y).min(1.0);
        let scale = self
            .zoom
            .filter(|zoom| zoom.is_finite() && *zoom > 0.0)
            .unwrap_or(fit);
        let pointer = pointer.unwrap_or(area.center());
        let snap = |value: f32| (value * pixels_per_point).round() / pixels_per_point;
        let x = snap(along(
            area.left(),
            area.width(),
            size.x * scale,
            pointer.x,
            0.5,
        ));
        let y = snap(along(
            area.top(),
            area.height(),
            size.y * scale,
            pointer.y,
            0.0,
        ));
        Some(Placement {
            scale,
            screen: Rect::from_min_size(pos2(x / scale, y / scale), size),
        })
    }
}

pub fn clamp_size(length: f32) -> f32 {
    match length.is_finite() {
        true => length.round().clamp(MINIMUM_SIZE, MAXIMUM_SIZE),
        false => MINIMUM_SIZE,
    }
}

pub fn around(outer: Rect, inner: Rect) -> [Rect; 4] {
    let inner = inner.intersect(outer);
    [
        Rect::from_min_max(outer.min, pos2(outer.right(), inner.top())),
        Rect::from_min_max(pos2(outer.left(), inner.bottom()), outer.max),
        Rect::from_min_max(
            pos2(outer.left(), inner.top()),
            pos2(inner.left(), inner.bottom()),
        ),
        Rect::from_min_max(
            pos2(inner.right(), inner.top()),
            pos2(outer.right(), inner.bottom()),
        ),
    ]
}

fn along(start: f32, available: f32, shown: f32, pointer: f32, slack: f32) -> f32 {
    if shown <= available {
        return start + (available - shown) * slack;
    }
    let fraction = ((pointer - start) / available).clamp(0.0, 1.0);
    start - fraction * (shown - available)
}
