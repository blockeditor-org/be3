//! Moving the selection on the drag board (plans/declarative_dragging.md, step 5): the
//! canvas is a `free` site whose area is its camera's world, unbounded and scrolled by the
//! edge band. The place the board chooses is the world point under the pointer; the move
//! applies it through the canvas's hold, so every view on this device follows it, and shows
//! it to peers as presence. The board's drop commits the hold; a put-down (Escape, a
//! cancelled press, an undo that ended the hold) drops it.

use std::rc::Rc;
use std::time::Duration;

use be_sdk_beui::beui::drag::{self, Carry, EdgeScroller};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{PointerId, PointerPress, Pos2, Rect, Vec2};
use be_type_infinite_canvas::CanvasPoint;
use uuid::Uuid;

use crate::geometry::{WorldRect, entity_bounds};

use super::state::{CanvasState, Gesture};

/// The selection of one canvas carried mid-move, by its block and where it was grabbed:
/// the grab's offset from the moved item's corner, in window points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CanvasMove {
    block: Uuid,
    grab: Vec2,
}

/// How fast the camera follows a carry held in the edge band, in points a second for each
/// point the pointer is into it.
const EDGE_SPEED: f32 = 12.0;

impl CanvasState {
    /// The carry a drag past the threshold lifts: the move a press on the selection began,
    /// or, from the keyboard, a move of the selection grabbed at the pointer the keyboard
    /// lifts at. The canvas draws what it carries itself.
    pub(crate) fn lift_move(&self, press: PointerPress) -> Option<Carry> {
        if self.previewing() {
            return None;
        }
        if press.pointer == PointerId::Keyboard {
            // Only between gestures, and only a selection something of which can move. The
            // board refuses a lift only while it carries something, so the move begun here is
            // lifted.
            if self.board().carrying()
                || self.focused_editor.get_untracked().is_some()
                || !self.tool.get_untracked().selects()
                || self.gesture.with_untracked(Option::is_some)
                || self.selected_unlocked().is_empty()
            {
                return None;
            }
            self.begin_move(self.world_at(press.pos), false);
        }
        let (start, bounds) = self.gesture.with_untracked(|gesture| match gesture {
            Some(Gesture::Move {
                start,
                originals,
                carried: false,
                ..
            }) => Some((
                *start,
                originals
                    .iter()
                    .map(entity_bounds)
                    .reduce(WorldRect::union)?,
            )),
            _ => None,
        })?;
        let camera = self.camera();
        let grabbed = camera.to_screen(Pos2::new(start.x, start.y));
        let item = camera
            .rect_to_screen(bounds.rect())
            .translate(-grabbed.to_vec2());
        Some(
            Carry::new(CanvasMove {
                block: self.block_id(),
                grab: item.min.to_vec2(),
            })
            .item(item)
            .drawn_by_site(),
        )
    }

    /// The board chose `at` for the carried move, the world point under the pointer, or
    /// nothing: the carry was put down, and the move goes back. A drop commits before it.
    fn carry_over(&self, at: Option<CanvasPoint>) {
        let carried = self
            .gesture
            .with_untracked(|gesture| matches!(gesture, Some(Gesture::Move { carried: true, .. })));
        match at {
            Some(at) => self.update_gesture(|gesture| {
                if let Gesture::Move {
                    current, carried, ..
                } = gesture
                {
                    *current = at;
                    *carried = true;
                }
            }),
            None if carried => self.begin_gesture(None),
            None => {}
        }
    }

    /// The board dropped the carried move at `at`: written as one step.
    fn drop_move(&self, at: CanvasPoint) {
        self.carry_over(Some(at));
        if self.take_gesture().is_some() {
            self.commit_writes();
        }
    }

    /// Whether the gesture is a move the board carries, which only the board ends.
    pub(crate) fn carries(&self) -> bool {
        self.gesture
            .with_untracked(|gesture| matches!(gesture, Some(Gesture::Move { carried: true, .. })))
    }
}

/// Makes the canvas the site its moves land in, and scrolls its camera while a move is held
/// in the edge band. `surface` is the node the pointer moves over.
pub(crate) fn use_move_site(state: CanvasState, surface: NodeRef) {
    let block = state.block_id();
    let site = use_drop_site(
        Func::new(move |carried: CanvasMove| carried.block == block),
        Func::new(move |carried: CanvasMove| {
            let Some(placed) = surface
                .try_get()
                .and_then(|node| measure(node).get_untracked())
            else {
                return drag::floating();
            };
            let origin = placed.rect.min;
            let scale = match placed.size.x > 0.0 {
                true => placed.rect.width() / placed.size.x,
                false => 1.0,
            };
            drag::free(surface, move |rect: Rect| {
                let pointer = origin + rect.min.to_vec2() * scale - carried.grab;
                state.world_at(pointer)
            })
            .unbounded()
            // A keyboard step is the arrows' nudge: a world unit, ten with Shift.
            .stepped(state.scale())
        }),
        Callback::new(move |(_, at): (CanvasMove, CanvasPoint)| state.drop_move(at)),
    );
    create_effect_on(
        move || site.over.get(),
        move |at: &Option<CanvasPoint>, _| untrack(|| state.carry_over(*at)),
    );
    edge_scroll(state, surface);
}

/// Pans the camera while a carried move is held in the surface's edge band, choosing again
/// as the world moves under the pointer.
fn edge_scroll(state: CanvasState, surface: NodeRef) {
    let block = state.block_id();
    let ours = move || carrying::<CanvasMove>().is_some_and(|carried| carried.block == block);
    let board = state.board();
    let speed = StoredCell::new(Vec2::ZERO);
    let ticked = StoredCell::new(now());
    let panning = create_timer(move || {
        let speed = speed.get();
        if speed == Vec2::ZERO {
            return None;
        }
        let elapsed = now().saturating_duration_since(ticked.replace(now()));
        state.editor().pan(-speed * elapsed.as_secs_f32());
        Some(Duration::ZERO)
    });
    let edge = board.register_scroller(EdgeScroller {
        id: 0,
        node: surface,
        movable: Rc::new(move || [[untrack(ours); 2]; 2]),
        scroll: Rc::new(move |depth: Option<Vec2>| {
            let now_speed = depth.map_or(Vec2::ZERO, |depth| depth * EDGE_SPEED);
            speed.set(now_speed);
            if now_speed == Vec2::ZERO {
                panning.stop();
            } else if !panning.running() {
                ticked.set(now());
                panning.start(Duration::ZERO);
            }
        }),
    });
    let unregister = Rc::clone(&board);
    on_cleanup(move || unregister.unregister_scroller(edge));
    // The world moved under a still pointer: the place under it is another.
    let camera = state.editor().canvas();
    create_effect_on(
        move || camera.track(),
        move |_, _| {
            if untrack(ours) {
                board.retrack();
            }
        },
    );
}
