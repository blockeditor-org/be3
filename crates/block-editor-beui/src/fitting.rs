use beui::reactive::*;
use beui::{Pos2, Rect, Vec2};

use crate::Editor;

const CENTRED: f32 = 0.5;

#[derive(Clone, Copy)]
pub struct Fitting {
    editor: Editor,
    content: Memo<Option<Rect>>,
}

#[derive(Clone, Copy)]
struct Fit {
    factor: f32,
    anchor: Pos2,
    delta: Vec2,
    scale: f32,
    centre: Pos2,
    at: Pos2,
}

impl Fit {
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

    fn fitted(&self, world: Rect, room: Rect, max_scale: f32) -> Option<Fit> {
        let view = self.canvas().get_untracked()?;
        let room = Rect::from_min_size(room.min, room.size().max(Vec2::splat(1.0)));
        let target = (room.width() / world.width().max(f32::EPSILON))
            .min(room.height() / world.height().max(f32::EPSILON))
            .min(max_scale);
        let anchor = view.to_screen(world.center());
        Some(Fit {
            factor: target / view.scale.max(f32::EPSILON),
            anchor,
            delta: room.center() - anchor,
            scale: target,
            centre: world.center(),
            at: room.center(),
        })
    }

    pub fn keep_fitted(&self, content: impl Fn() -> Option<Rect> + 'static) -> Fitting {
        let fitting = Fitting {
            editor: *self,
            content: create_memo(content),
        };
        let canvas = self.canvas();
        let moved = self.moved();
        let band = self.placed();
        create_effect(move || {
            if moved.get() || !fitting.framed() {
                return;
            }
            let band = band.get();
            if let (Some(_), Some(world)) = (canvas.get(), fitting.content.get())
                && band.is_positive()
            {
                untrack(|| fitting.follow(world, band, false));
            }
        });
        fitting
    }
}

impl Fitting {
    pub fn fit(&self) {
        let editor = self.editor;
        let Some(world) = self.content.get_untracked() else {
            return;
        };
        let band = editor.placed().get_untracked();
        match untrack(|| self.framed()) {
            true => self.follow(world, band, true),
            false => {
                editor.fit(world, band, f32::INFINITY);
            }
        }
    }

    fn framed(&self) -> bool {
        self.editor.chrome_shown().get() && self.editor.placed_content().get().is_none()
    }

    fn follow(&self, world: Rect, band: Rect, resume: bool) {
        let Some(fit) = self.editor.fitted(world, band, 1.0) else {
            return;
        };
        if resume || fit.zooms() || fit.pans() {
            self.editor.follow(fit.scale, fit.centre, fit.at, resume);
        }
    }
}
