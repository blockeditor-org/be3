use crate::geometry::{Pos2, Rect, pos2};

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct ScreenSimulation {
    pub(crate) size: f32,
    pub(crate) zoom: Option<f32>,
}

impl Default for ScreenSimulation {
    fn default() -> Self {
        Self {
            size: 1.0,
            zoom: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Placement {
    pub(crate) scale: f32,
    pub(crate) screen: Rect,
}

impl ScreenSimulation {
    pub(crate) fn scale(self) -> Option<f32> {
        if !self.size.is_finite() || self.size <= 0.0 {
            return None;
        }
        let scale = self
            .zoom
            .filter(|zoom| zoom.is_finite() && *zoom > 0.0)
            .unwrap_or(self.size.recip());
        (self.size != 1.0 || scale != 1.0).then_some(scale)
    }

    pub(crate) fn place(self, shown: Rect, pointer: Option<Pos2>) -> Option<Placement> {
        let scale = self.scale()?;
        if !shown.is_positive() {
            return None;
        }
        let size = shown.size() * self.size;
        let pointer = pointer.unwrap_or(shown.center());
        let x = along(shown.left(), shown.width(), size.x * scale, pointer.x);
        let y = along(shown.top(), shown.height(), size.y * scale, pointer.y);
        Some(Placement {
            scale,
            screen: Rect::from_min_size(pos2(x / scale, y / scale), size),
        })
    }
}

fn along(start: f32, available: f32, shown: f32, pointer: f32) -> f32 {
    if shown <= available {
        return start + (available - shown) / 2.0;
    }
    let fraction = ((pointer - start) / available).clamp(0.0, 1.0);
    start - fraction * (shown - available)
}
