use block_plugin_api::FitArea;
use geometry::{Pos2, Rect, Vec2};

use super::EditorCore;

const CENTRED: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fit {
    World,
    Centre,
    Area(Rect),
}

#[derive(Clone, Copy)]
struct Zoomed {
    factor: f32,
    anchor: Pos2,
    delta: Vec2,
}

impl Zoomed {
    fn zooms(&self) -> bool {
        (self.factor - 1.0).abs() > 1e-4
    }

    fn pans(&self) -> bool {
        self.delta.length() > CENTRED * 0.5
    }
}

impl EditorCore {
    pub fn fit(&self, world: Rect, room: Rect, max_scale: f32) -> bool {
        let Some(fit) = self.fitted(world, room, max_scale) else {
            return false;
        };
        if fit.zooms() {
            self.zoom_at(fit.factor, fit.anchor);
        }
        if fit.pans() {
            self.pan(fit.delta);
        }
        fit.zooms() || fit.pans()
    }

    fn fitted(&self, world: Rect, room: Rect, max_scale: f32) -> Option<Zoomed> {
        let view = self.0.shown.borrow().canvas?;
        let room = Rect::from_min_size(room.min, room.size().max(Vec2::splat(1.0)));
        let target = (room.width() / world.width().max(f32::EPSILON))
            .min(room.height() / world.height().max(f32::EPSILON))
            .min(max_scale);
        let anchor = view.to_screen(world.center());
        Some(Zoomed {
            factor: target / view.scale.max(f32::EPSILON),
            anchor,
            delta: room.center() - anchor,
        })
    }

    pub fn fit_area(&self, fit: Fit) -> FitArea {
        match fit {
            Fit::World => FitArea::World,
            Fit::Centre => FitArea::Centre,
            Fit::Area(rect) => FitArea::Rect(self.to_host(rect)),
        }
    }

    pub fn fit_now(&self, fit: Option<Fit>) {
        let shown = self.shown();
        if shown.chrome && shown.placed_content.is_none() {
            self.fit_view();
            return;
        }
        let (Some(fit), Some(world)) = (fit, shown.world) else {
            return;
        };
        let world = match fit {
            Fit::World => Rect::from_min_size(Pos2::ZERO, world),
            Fit::Centre => {
                Rect::from_center_size(Pos2::new(world.x * 0.5, world.y * 0.5), Vec2::ZERO)
            }
            Fit::Area(rect) => rect,
        };
        self.fit(world, shown.placed, f32::INFINITY);
    }
}
