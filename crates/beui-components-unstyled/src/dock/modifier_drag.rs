use std::cell::Cell;
use std::rc::Rc;

use beui_core::drag_board::{DragPoint, Pending};
use beui_core::input::{Modifiers, SecondaryDrag};

use super::{
    DockDragged, DockDrop, DockLayout, DockPreviewHandle, DockState, Entry, FLOATING_SIZE,
    GRAB_OFFSET, Grip, Handle, LeafId, SplitId, State, SurfaceId, TabId, fraction_moved,
    layout_tree,
};
use crate::drag::{DRAG_PREVIEW_OFFSET, DRAG_THRESHOLD};
use beui_core::base::Direction;
use beui_core::base::ItemSize;
use beui_core::base::overlay::{OverlayAnchor, OverlayMode, Placement};
use beui_core::geometry::{Pos2, Rect, Vec2};
use beui_core::input::PointerPress;
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    Dynamic, Frame, Interactive, IntoProp, List, Memo, Portal, ReadSignal, clone, create_memo,
    create_signal, on_cleanup, with_document,
};

const EDGE_MARGIN: f32 = 4.0;

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
    pub(super) fn carried_window(
        &self,
        state: &DockState,
        dragged: DockDragged,
    ) -> (Option<SurfaceId>, Vec2, Vec2) {
        let held = self.grab.get().and_then(|grab| {
            let surface = lone_window(state, dragged)?;
            Some((surface, grab, state.window_rect(surface)?.size()))
        });
        match held {
            Some((surface, grab, size)) => (Some(surface), grab, size),
            None => (None, GRAB_OFFSET, FLOATING_SIZE),
        }
    }

    pub(super) fn floats(&self, modifiers: Modifiers) -> bool {
        modifiers.alt
            && !self
                .drag_modifier
                .get_untracked()
                .is_some_and(|held| held.alt)
    }

    fn resize_target(&self, leaf: LeafId, pos: Pos2) -> Option<Resize> {
        let state = self.state.get_untracked();
        let surface = state.surface_of(leaf)?;
        if let Some(start) = state.window_rect(surface) {
            let shown = self.surface_rect(surface)?;
            return Some(Resize::Window {
                surface,
                grip: nearest_grip(shown, pos),
                start,
            });
        }
        let trees: Vec<_> = self.panes.borrow().keys().copied().collect();
        let layouts: Vec<DockLayout> = trees
            .into_iter()
            .filter_map(|tree| {
                let area = self.pane_rect(tree)?;
                let layout = layout_tree(&state, tree, area, self.thickness);
                layout
                    .leaves
                    .iter()
                    .any(|(other, _)| state.surface_of(*other) == Some(surface))
                    .then_some(layout)
            })
            .collect();
        let rect = layouts.iter().find_map(|layout| layout.leaf_rect(leaf))?;
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
                let fraction = fraction_moved(
                    area,
                    direction,
                    self.thickness,
                    start,
                    direction.main(moved),
                );
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

pub(super) fn vacant_target(state: &DockState, target: Option<DockDrop>) -> bool {
    matches!(target, Some(DockDrop::Pane { leaf }) if state.entries(leaf).is_empty())
}

fn lone_window(state: &DockState, dragged: DockDragged) -> Option<SurfaceId> {
    let DockDragged::Entry(Entry::Tab(tab)) = dragged else {
        return None;
    };
    let surface = state
        .windows()
        .into_iter()
        .find(|surface| state.surface_tabs(*surface) == [tab])?;
    let leaf = state.find(tab)?.leaf;
    (!state.is_nested(leaf)).then_some(surface)
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

#[component]
pub(super) fn ModifierDrag(
    dock: Handle,
    leaf: LeafId,
    shown: Memo<Option<TabId>>,
    panel: ReadSignal<Option<NodeId>>,
) -> NodeId {
    let held = dock.drag_modifier.clone();
    let board = with_document(|document| document.drag_board());
    let pressed_at: Rc<Cell<Option<Pos2>>> = Rc::new(Cell::new(None));
    let (carried, set_carried) = create_signal(None::<DockDragged>);
    let (pointer, set_pointer) = create_signal(Pos2::ZERO);
    let moved: Pending = Rc::new(
        clone!(dock board pressed_at carried set_carried shown -> move |point: DragPoint| {
            let Some(origin) = pressed_at.get() else {
                return;
            };
            if carried.with_untracked(Option::is_some)
                || (point.pos - origin).length() < DRAG_THRESHOLD
                || board.carrying()
            {
                return;
            }
            let Some(tab) = shown.get_untracked() else {
                return;
            };
            let dragged = DockDragged::Entry(Entry::Tab(tab));
            let state = dock.state.get_untracked();
            let grab = lone_window(&state, dragged)
                .and_then(|surface| dock.surface_rect(surface))
                .map(|window| origin - window.min);
            dock.grab.set(grab);
            let follow = set_pointer.clone();
            set_carried.set(Some(dragged));
            dock.begin_drag(dragged);
            board.begin(Rc::new(dragged), point, Rc::new(move |pos| follow.set(pos)));
        }),
    );
    let pressed = clone!(board pressed_at moved held -> move |press: PointerPress| {
        if !held.get_untracked().is_some_and(|held| press.modifiers.holds(held)) {
            return;
        }
        pressed_at.set(Some(press.pos));
        board.press(Rc::clone(&moved));
    });
    let dragged = clone!(moved -> move |press: PointerPress| moved(DragPoint::of(press)));
    let released = clone!(dock board pressed_at carried set_carried -> move |active: bool| {
        if active || pressed_at.take().is_none() {
            return;
        }
        board.release();
        if carried.with_untracked(Option::is_none) {
            return;
        }
        board.finish();
        set_carried.set(None);
        dock.end_drag();
        dock.grab.set(None);
    });
    on_cleanup(clone!(dock board carried -> move || {
        if carried.with_untracked(Option::is_some) {
            board.end(None);
            dock.grab.set(None);
        }
    }));
    let resizing: Rc<Cell<Option<Resize>>> = Rc::new(Cell::new(None));
    let resized = clone!(dock held -> move |drag: SecondaryDrag| {
        if drag.started {
            let target = held
                .get_untracked()
                .filter(|held| drag.modifiers.holds(*held))
                .and_then(|_| dock.resize_target(leaf, drag.from));
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
    });
    let titled = dock.clone();
    let title = create_memo(clone!(shown -> move || {
        shown.get().map(|tab| titled.title(tab)).unwrap_or_default()
    }));
    let pictured = dock.clone();
    let icon = create_memo(move || {
        shown
            .get()
            .map(|tab| pictured.icon(tab))
            .unwrap_or_default()
    });
    let preview = dock.preview.clone();
    let dragging = create_memo(clone!(carried -> move || carried.with(Option::is_some)));
    let anchor = create_memo(move || pointer.get() + DRAG_PREVIEW_OFFSET)
        .into_prop()
        .map(OverlayAnchor::Point);
    view! {
        <Interactive
            claim_modifiers={held}
            claims_touch=false
            on_press={pressed}
            on_drag={dragged}
            on_active_change={released}
            on_secondary_drag={resized}
        >
            <List spacing=0.0>
                <Portal node={panel} @sizing=ItemSize::Percent(100.0) />
                <Overlay
                    anchor={anchor}
                    placement=Placement::BelowStart
                    mode=OverlayMode::Passive
                    traps_focus=false
                    open={dragging}
                >
                    <List spacing=0.0>
                        <Dynamic value={carried}>
                            {move |carried: Option<DockDragged>| match carried {
                                Some(dragged) => preview.call(DockPreviewHandle {
                                    dragged,
                                    title: title.clone(),
                                    icon: icon.clone(),
                                }),
                                None => view! {
                                    <Frame />
                                },
                            }}
                        </Dynamic>
                    </List>
                </Overlay>
            </List>
        </Interactive>
    }
}
