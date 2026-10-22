use std::cell::Cell;

use beui::Rect;
use beui::reactive::*;

use crate::Editor;

const CENTRED: f32 = 0.5;
const RETRIES: u8 = 2;

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Asked {
        from: Option<CanvasView>,
        band: Rect,
        world: Rect,
        tries: u8,
    },
    Resting {
        view: CanvasView,
        band: Rect,
        world: Rect,
    },
    Moved,
}

const START: Phase = Phase::Asked {
    from: None,
    band: Rect::ZERO,
    world: Rect::ZERO,
    tries: 0,
};

#[derive(Clone, Copy)]
pub struct Fitting {
    editor: Editor,
    content: Memo<Option<Rect>>,
    phase: Stored<Cell<Phase>>,
}

impl Editor {
    pub fn fit(&self, world: Rect, room: Rect, max_scale: f32) -> bool {
        let Some(view) = self.canvas().get_untracked() else {
            return false;
        };
        let room = Rect::from_min_size(room.min, room.size().max(beui::Vec2::splat(1.0)));
        let target = (room.width() / world.width().max(f32::EPSILON))
            .min(room.height() / world.height().max(f32::EPSILON))
            .min(max_scale);
        let factor = target / view.scale.max(f32::EPSILON);
        let anchor = view.to_screen(world.center());
        let delta = room.center() - anchor;
        let zooms = (factor - 1.0).abs() > 1e-4;
        let pans = delta.length() > CENTRED * 0.5;
        if zooms {
            self.zoom_at(factor, anchor);
        }
        if pans {
            self.pan(delta);
        }
        zooms || pans
    }

    pub fn keep_fitted(&self, content: impl Fn() -> Option<Rect> + 'static) -> Fitting {
        let fitting = Fitting {
            editor: *self,
            content: create_memo(content),
            phase: store(Cell::new(START)),
        };
        let canvas = self.canvas();
        let band = self.placed();
        let chrome = self.chrome_shown();
        let placed = self.placed_content();
        create_effect(move || {
            let view = canvas.get();
            let band = band.get();
            let world = fitting.content.get();
            let own = chrome.get() && placed.get().is_none();
            if let (Some(view), Some(world)) = (view, world)
                && band.is_positive()
            {
                untrack(|| fitting.step(view, band, world, own));
            }
        });
        fitting
    }
}

impl Fitting {
    pub fn fit(&self) {
        let editor = self.editor;
        let (Some(view), Some(world)) = (
            editor.canvas().get_untracked(),
            self.content.get_untracked(),
        ) else {
            return;
        };
        let band = editor.placed().get_untracked();
        let own = editor.chrome_shown().get_untracked()
            && editor.placed_content().get_untracked().is_none();
        if !own {
            editor.fit(world, band, f32::INFINITY);
            return;
        }
        self.phase.value().set(START);
        self.step(view, band, world, true);
    }

    fn step(&self, view: CanvasView, band: Rect, world: Rect, own: bool) {
        let phase = self.phase.value();
        let next = match phase.get() {
            Phase::Moved => Phase::Moved,
            Phase::Resting {
                view: rest,
                band: was,
                world: had,
            } => {
                let same = near(band, was) && near(world, had);
                if same && view != rest {
                    Phase::Moved
                } else if same {
                    return;
                } else if own {
                    self.ask(view, band, world, 0)
                } else {
                    Phase::Resting { view, band, world }
                }
            }
            Phase::Asked {
                from,
                band: was,
                world: had,
                tries,
            } => {
                if !own {
                    Phase::Resting { view, band, world }
                } else if from.is_none() || !near(band, was) || !near(world, had) {
                    self.ask(view, band, world, 0)
                } else if Some(view) == from {
                    return;
                } else if centred(view, band, world) || tries >= RETRIES {
                    Phase::Resting { view, band, world }
                } else {
                    self.ask(view, band, world, tries + 1)
                }
            }
        };
        phase.set(next);
    }

    fn ask(&self, view: CanvasView, band: Rect, world: Rect, tries: u8) -> Phase {
        match self.editor.fit(world, band, 1.0) {
            true => Phase::Asked {
                from: Some(view),
                band,
                world,
                tries,
            },
            false => Phase::Resting { view, band, world },
        }
    }
}

fn centred(view: CanvasView, band: Rect, world: Rect) -> bool {
    (view.to_screen(world.center()) - band.center()).length() < CENTRED
}

fn near(a: Rect, b: Rect) -> bool {
    (a.min - b.min).length() < CENTRED && (a.max - b.max).length() < CENTRED
}
