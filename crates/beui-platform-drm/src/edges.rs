//! Screen edges a program reveals something from (a desktop's auto-hidden bars): walls the pointer
//! pushes against, and strips a finger swipes in from. The program says where they are
//! (`ScreenEdges::set`) and hears how far each was pushed or pulled (`ScreenEdges::states`); what
//! to reveal, and when, is its own business.

use std::cell::RefCell;
use std::rc::Rc;

use beui::{Pos2, Rect, Vec2};

use crate::layout::{inside, moved};

/// How long the pointer rests against a deliberate edge before its pushes count, so a sweep that
/// ends at the edge never counts, however fast it was.
const SETTLE_USEC: u64 = 150_000;
/// How far in from an edge a finger may land to swipe it, in points.
const TOUCH_REACH: f32 = 12.0;
/// The same, over a full screen program, whose own controls may sit near its edges.
const DELIBERATE_TOUCH_REACH: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

/// An edge of one display, from `start` to `end` along it, in desktop points.
///
/// With `wall`, the pointer cannot cross it, whether the desktop ends there or another display
/// begins, and pushing on past it counts toward `EdgeState::pushed`; a finger landing just inside
/// it swipes it (`EdgeState::pull`) and never reaches what is under it. Without, it is open:
/// `keep` is then where the pointer counts as near what it revealed (`EdgeState::near`).
///
/// `deliberate` (over a full screen program) counts pushes only once the pointer has rested
/// against the wall a moment, and takes swipes only from closer to the edge.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenEdge {
    pub id: u32,
    pub display: Rect,
    pub side: Side,
    pub start: f32,
    pub end: f32,
    pub wall: bool,
    pub deliberate: bool,
    pub keep: Option<Rect>,
}

/// What happened at an edge since it was set. `pushed` only grows, in points, so a reader that
/// misses some changes still sees all of it. `pull` is how far a finger swiping in from the edge
/// is from it; when the finger lifts, `pulls` counts one more and `pulled` is how far it reached
/// (nothing for a cancelled swipe). `taps_away` counts fingers that landed outside `keep` while
/// the edge was open, which is how a touch says it is done with what the edge revealed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EdgeState {
    pub id: u32,
    pub pushed: f32,
    pub near: bool,
    pub pull: Option<f32>,
    pub pulls: u32,
    pub pulled: f32,
    pub taps_away: u32,
}

#[derive(Clone, Debug)]
struct Held {
    edge: ScreenEdge,
    state: EdgeState,
    /// When the pointer came to rest against the wall, while it stays there.
    against: Option<u64>,
}

/// The edges and what they heard, without the platform: the runner feeds it pointer motion and
/// touches, and it says where the pointer goes and which touches it took.
#[derive(Debug, Default)]
pub(crate) struct Edges {
    held: Vec<Held>,
    touches: Vec<(i32, u32)>,
    pointer: Option<Pos2>,
}

impl Edges {
    pub(crate) fn set(&mut self, edges: Vec<ScreenEdge>) {
        let mut before = std::mem::take(&mut self.held);
        self.held = edges
            .into_iter()
            .map(|edge| {
                let kept = before
                    .iter()
                    .position(|held| held.edge.id == edge.id)
                    .map(|at| before.swap_remove(at));
                let (state, against) = match kept {
                    Some(held) => (held.state, held.against.filter(|_| edge.wall)),
                    None => (
                        EdgeState {
                            id: edge.id,
                            ..EdgeState::default()
                        },
                        None,
                    ),
                };
                Held {
                    edge,
                    state,
                    against,
                }
            })
            .collect();
        let held = &self.held;
        self.touches.retain(|(_, id)| {
            held.iter()
                .any(|held| held.edge.id == *id && held.edge.wall)
        });
        if let Some(pointer) = self.pointer {
            self.near(pointer);
        }
    }

    pub(crate) fn states(&self) -> Vec<EdgeState> {
        self.held.iter().map(|held| held.state.clone()).collect()
    }

    /// Moves the pointer by `delta` among `outputs`, stopped by every wall it would cross.
    pub(crate) fn moved(
        &mut self,
        position: Pos2,
        delta: Vec2,
        time: u64,
        outputs: &[Rect],
    ) -> Pos2 {
        let mut target = position + delta;
        let mut pushes = Vec::new();
        for (at, held) in self.held.iter().enumerate() {
            let edge = &held.edge;
            if !edge.wall || !on_edge(edge, position) {
                continue;
            }
            let limit = limit(edge);
            let excess = match edge.side {
                Side::Top => limit - target.y,
                Side::Bottom => target.y - limit,
                Side::Left => limit - target.x,
                Side::Right => target.x - limit,
            };
            if excess <= 0.0 {
                continue;
            }
            match edge.side {
                Side::Top | Side::Bottom => target.y = limit,
                Side::Left | Side::Right => target.x = limit,
            }
            pushes.push((at, excess, against(edge, position)));
        }
        let placed = moved(position, target - position, outputs);
        for (at, excess, resting) in pushes {
            let held = &mut self.held[at];
            match (resting, held.against) {
                (true, Some(since)) => {
                    if !held.edge.deliberate || time.saturating_sub(since) >= SETTLE_USEC {
                        held.state.pushed += excess;
                    }
                }
                (true, None) | (false, _) => held.against = Some(time),
            }
        }
        self.placed(placed);
        placed
    }

    /// The pointer is at `pointer`, however it got there.
    pub(crate) fn placed(&mut self, pointer: Pos2) {
        self.pointer = Some(pointer);
        for held in &mut self.held {
            if !held.edge.wall || !against(&held.edge, pointer) {
                held.against = None;
            }
        }
        self.near(pointer);
    }

    fn near(&mut self, pointer: Pos2) {
        for held in &mut self.held {
            held.state.near = held.edge.keep.is_some_and(|keep| keep.contains(pointer));
        }
    }

    /// A finger landed at `at`: true when it swipes an edge, which then has it until it lifts.
    pub(crate) fn touch_down(&mut self, finger: i32, at: Pos2) -> bool {
        self.touches.retain(|(held, _)| *held != finger);
        for held in &mut self.held {
            if held.edge.keep.is_some_and(|keep| !keep.contains(at)) {
                held.state.taps_away += 1;
            }
        }
        let Some(held) = self.held.iter_mut().find(|held| {
            let edge = &held.edge;
            let reach = match edge.deliberate {
                true => DELIBERATE_TOUCH_REACH,
                false => TOUCH_REACH,
            };
            edge.wall && on_edge(edge, at) && inward(edge, at) < reach
        }) else {
            return false;
        };
        held.state.pull = Some(inward(&held.edge, at).max(0.0));
        self.touches.push((finger, held.edge.id));
        true
    }

    /// A finger moved: true when an edge has it.
    pub(crate) fn touch_moved(&mut self, finger: i32, at: Pos2) -> bool {
        let Some(held) = self.swiping(finger) else {
            return false;
        };
        held.state.pull = Some(inward(&held.edge, at).max(0.0));
        true
    }

    /// A finger lifted, or its touch was cancelled: true when an edge had it.
    pub(crate) fn touch_up(&mut self, finger: i32, cancelled: bool) -> bool {
        let Some(held) = self.swiping(finger) else {
            return false;
        };
        let reached = held.state.pull.take().unwrap_or_default();
        held.state.pulls += 1;
        held.state.pulled = match cancelled {
            true => 0.0,
            false => reached,
        };
        self.touches.retain(|(held, _)| *held != finger);
        true
    }

    fn swiping(&mut self, finger: i32) -> Option<&mut Held> {
        let id = self
            .touches
            .iter()
            .find(|(held, _)| *held == finger)
            .map(|(_, id)| *id)?;
        self.held.iter_mut().find(|held| held.edge.id == id)
    }
}

/// The last row or column of the display along the edge, where the pointer stops.
fn limit(edge: &ScreenEdge) -> f32 {
    let display = edge.display;
    match edge.side {
        Side::Top => display.top(),
        Side::Bottom => (display.bottom() - 1.0).max(display.top()),
        Side::Left => display.left(),
        Side::Right => (display.right() - 1.0).max(display.left()),
    }
}

fn along(edge: &ScreenEdge, at: Pos2) -> f32 {
    match edge.side {
        Side::Top | Side::Bottom => at.x,
        Side::Left | Side::Right => at.y,
    }
}

fn on_edge(edge: &ScreenEdge, at: Pos2) -> bool {
    let along = along(edge, at);
    inside(edge.display, at) && along >= edge.start && along < edge.end
}

fn inward(edge: &ScreenEdge, at: Pos2) -> f32 {
    let display = edge.display;
    match edge.side {
        Side::Top => at.y - display.top(),
        Side::Bottom => display.bottom() - at.y,
        Side::Left => at.x - display.left(),
        Side::Right => display.right() - at.x,
    }
}

fn against(edge: &ScreenEdge, at: Pos2) -> bool {
    let limit = limit(edge);
    let across = match edge.side {
        Side::Top | Side::Bottom => at.y,
        Side::Left | Side::Right => at.x,
    };
    on_edge(edge, at) && (across - limit).abs() < 0.01
}

#[derive(Default)]
struct Shared {
    edges: RefCell<Edges>,
    watchers: RefCell<Vec<Rc<dyn Fn()>>>,
}

/// The screen edges, handed to the program through `Setup`.
#[derive(Clone, Default)]
pub struct ScreenEdges(Rc<Shared>);

impl ScreenEdges {
    /// Replaces every edge. An edge keeps what it heard while its `id` stays.
    pub fn set(&self, edges: Vec<ScreenEdge>) {
        self.change(|held| held.set(edges));
    }

    pub fn states(&self) -> Vec<EdgeState> {
        self.0.edges.borrow().states()
    }

    /// Hears every change to `states`.
    pub fn watch(&self, watcher: impl Fn() + 'static) {
        self.0.watchers.borrow_mut().push(Rc::new(watcher));
    }

    pub(crate) fn change<R>(&self, change: impl FnOnce(&mut Edges) -> R) -> R {
        let (result, changed) = {
            let mut edges = self.0.edges.borrow_mut();
            let before = edges.states();
            let result = change(&mut edges);
            (result, edges.states() != before)
        };
        if changed {
            let watchers = self.0.watchers.borrow().clone();
            for watcher in watchers {
                watcher();
            }
        }
        result
    }
}

#[cfg(test)]
mod tests;
