use beui::reactive::*;
use beui::{Pos2, Rect, Vec2};
use block_plugin_api::FitArea;

use crate::Editor;

const CENTRED: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fit {
    World,
    Centre,
    Area(Rect),
}

#[derive(Clone, Copy)]
pub struct Fitting {
    editor: Editor,
    fit: Memo<Option<Fit>>,
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

impl Editor {
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
        let view = self.canvas().get_untracked()?;
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

    pub fn keep_fitted(&self, fit: impl Fn() -> Option<Fit> + 'static) -> Fitting {
        let editor = *self;
        let fit = create_memo(fit);
        let world = self.world();
        let area = create_memo(move || {
            let _ = world.get();
            fit.get().map(|fit| match fit {
                Fit::World => FitArea::World,
                Fit::Centre => FitArea::Centre,
                Fit::Area(rect) => editor.host_rect(rect),
            })
        });
        create_effect(move || {
            let area = area.get();
            untrack(|| editor.keep_fitted_area(area));
        });
        Fitting { editor, fit }
    }
}

impl Fitting {
    pub fn fit(&self) {
        let editor = self.editor;
        if untrack(|| editor.chrome_shown().get() && editor.placed_content().get().is_none()) {
            editor.fit_view();
            return;
        }
        let (Some(fit), Some(world)) = (self.fit.get_untracked(), editor.world().get_untracked())
        else {
            return;
        };
        let world = match fit {
            Fit::World => Rect::from_min_size(Pos2::ZERO, world),
            Fit::Centre => {
                Rect::from_center_size(Pos2::new(world.x * 0.5, world.y * 0.5), Vec2::ZERO)
            }
            Fit::Area(rect) => rect,
        };
        editor.fit(world, editor.placed().get_untracked(), f32::INFINITY);
    }
}
