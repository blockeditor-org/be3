use std::cell::Cell;
use std::rc::Rc;

use beui_core::base::Direction;
use beui_core::geometry::{Pos2, Rect, Vec2};
use beui_core::input::{Modifiers, SecondaryDrag};

use super::{DockLayout, Grip, Handle, SplitId, State, SurfaceId, fraction_moved, layout_tree};

const EDGE_MARGIN: f32 = 4.0;

#[derive(Clone, Copy)]
pub(super) enum Resized {
    Window(SurfaceId),
    Docked,
}

#[derive(Clone, Copy)]
enum Resize {
    Split {
        split: SplitId,
        direction: Direction,
        area: Rect,
        start: f32,
    },
    Window {
        surface: SurfaceId,
        grip: Grip,
        start: Rect,
    },
}

impl State {
    pub(super) fn modifier_held(&self, modifiers: Modifiers) -> bool {
        self.drag_modifier
            .get_untracked()
            .is_some_and(|held| held.any() && modifiers.holds(held))
    }

    fn resize_target(&self, resized: Resized, pos: Pos2) -> Option<Resize> {
        let state = self.state.get_untracked();
        if let Resized::Window(surface) = resized {
            let start = state.window_rect(surface)?;
            let shown = self.surface_rect(surface)?;
            return Some(Resize::Window {
                surface,
                grip: nearest_grip(shown, pos),
                start,
            });
        }
        let main = state.main();
        let trees: Vec<_> = self.panes.borrow().keys().copied().collect();
        let layouts: Vec<DockLayout> = trees
            .into_iter()
            .filter_map(|tree| {
                let area = self.pane_rect(tree)?;
                let layout = layout_tree(&state, tree, area, self.thickness);
                layout
                    .leaves
                    .iter()
                    .any(|(leaf, _)| state.surface_of(*leaf) == Some(main))
                    .then_some(layout)
            })
            .collect();
        let rect = layouts
            .iter()
            .flat_map(|layout| layout.leaves.iter().map(|(_, rect)| *rect))
            .filter(|rect| rect.contains(pos))
            .min_by(|first, second| {
                (first.width() * first.height()).total_cmp(&(second.width() * second.height()))
            })?;
        let margin = self.group_inset + self.thickness + EDGE_MARGIN;
        let reach = rect.expand(margin);
        layouts
            .iter()
            .flat_map(|layout| layout.splitters.iter())
            .filter(|splitter| {
                let shared = reach.intersect(splitter.handle);
                shared.is_positive() && shared.width().max(shared.height()) > margin * 2.0
            })
            .min_by(|first, second| {
                distance(first.handle, pos).total_cmp(&distance(second.handle, pos))
            })
            .map(|splitter| Resize::Split {
                split: splitter.id,
                direction: splitter.direction,
                area: splitter.area,
                start: splitter.fraction,
            })
    }

    fn resize(&self, target: Resize, moved: Vec2) {
        match target {
            Resize::Split {
                split,
                direction,
                area,
                start,
            } => {
                let fraction =
                    fraction_moved(area, direction, self.thickness, start, direction.main(moved));
                self.edit(|state| state.set_split_fraction(split, fraction));
            }
            Resize::Window {
                surface,
                grip,
                start,
            } => {
                let resized = grip.resized(start, moved);
                self.edit(|state| state.set_window_rect(surface, resized));
            }
        }
    }
}

pub(super) fn modifier_resize(dock: &Handle, resized: Resized) -> impl Fn(SecondaryDrag) + 'static {
    let dock = Rc::clone(dock);
    let resizing: Rc<Cell<Option<Resize>>> = Rc::new(Cell::new(None));
    move |drag: SecondaryDrag| {
        if drag.started {
            let target = dock
                .modifier_held(drag.modifiers)
                .then(|| dock.resize_target(resized, drag.from))
                .flatten();
            resizing.set(target);
        }
        let Some(target) = resizing.get() else {
            return;
        };
        let moved = match drag.cancelled {
            true => Vec2::ZERO,
            false => drag.pos - drag.from,
        };
        dock.resize(target, moved);
        if drag.ended || drag.cancelled {
            resizing.set(None);
        }
    }
}

fn distance(rect: Rect, pos: Pos2) -> f32 {
    let x = (rect.left() - pos.x).max(pos.x - rect.right()).max(0.0);
    let y = (rect.top() - pos.y).max(pos.y - rect.bottom()).max(0.0);
    Vec2::new(x, y).length()
}

fn third(at: f32, start: f32, length: f32) -> i8 {
    let fraction = (at - start) / length.max(1.0);
    match fraction {
        fraction if fraction < 1.0 / 3.0 => -1,
        fraction if fraction > 2.0 / 3.0 => 1,
        _ => 0,
    }
}

fn nearest_grip(rect: Rect, pos: Pos2) -> Grip {
    let across = third(pos.x, rect.left(), rect.width());
    let down = third(pos.y, rect.top(), rect.height());
    match (across, down) {
        (-1, -1) => Grip::TopLeft,
        (0, -1) => Grip::Top,
        (1, -1) => Grip::TopRight,
        (-1, 0) => Grip::Left,
        (1, 0) => Grip::Right,
        (-1, 1) => Grip::BottomLeft,
        (0, 1) => Grip::Bottom,
        (1, 1) => Grip::BottomRight,
        _ => [
            (pos.x - rect.left(), Grip::Left),
            (rect.right() - pos.x, Grip::Right),
            (pos.y - rect.top(), Grip::Top),
            (rect.bottom() - pos.y, Grip::Bottom),
        ]
        .into_iter()
        .min_by(|(first, _), (second, _)| first.total_cmp(second))
        .map_or(Grip::BottomRight, |(_, grip)| grip),
    }
}
