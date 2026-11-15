//! Walls and regions a program (a desktop's shell) watches the pointer and fingers at: walls
//! relative pointer motion cannot cross, and rects where it hears the pointer and fingers and may
//! take the fingers that land there. The program sets them (`ScreenInput::set`) and hears what
//! happened at each (`ScreenInput::state`); what that means, and what to show for it, is its own
//! business.

use std::cell::RefCell;
use std::rc::Rc;

use beui::{Pos2, Rect, Vec2};

use crate::layout::{inside, moved};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

/// A stretch of `side` of `display`, from `start` to `end` along it, in desktop points, that
/// relative pointer motion cannot cross, whether the desktop ends there or another display begins.
#[derive(Clone, Debug, PartialEq)]
pub struct Wall {
    pub id: u32,
    pub display: Rect,
    pub side: Side,
    pub start: f32,
    pub end: f32,
}

/// A rect of the desktop where the program hears the pointer and fingers. With `captures_touch`,
/// a finger landing in it is the program's until it lifts and never reaches a window.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub id: u32,
    pub rect: Rect,
    pub captures_touch: bool,
}

/// `against`: the pointer rests against the wall. `pushed`: how far relative motion that began
/// with the pointer against it has run on past it, in all, in points; it only grows, so a reader
/// that misses changes still sees all of it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WallState {
    pub id: u32,
    pub against: bool,
    pub pushed: f32,
}

/// `pointer`: the pointer is in the region. `touch_downs` counts fingers that landed in it.
/// `touch` is where the finger it captured is, while one is down; when that finger lifts,
/// `lifts` counts one more and `lifted` is where it lifted (nothing when it was cancelled).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RegionState {
    pub id: u32,
    pub pointer: bool,
    pub touch_downs: u32,
    pub touch: Option<Pos2>,
    pub lifts: u32,
    pub lifted: Option<Pos2>,
}

/// Every wall and region as it stands, and `touch_downs`, every finger that landed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScreenInputState {
    pub walls: Vec<WallState>,
    pub regions: Vec<RegionState>,
    pub touch_downs: u32,
}

/// The walls and regions and what they heard, without the platform: the runner feeds it pointer
/// motion and touches, and it says where the pointer goes and which touches it took.
#[derive(Debug, Default)]
pub(crate) struct Input {
    walls: Vec<(Wall, WallState)>,
    regions: Vec<(Region, RegionState)>,
    touch_downs: u32,
    /// Fingers a region captured, with the region's id.
    touches: Vec<(i32, u32)>,
    pointer: Option<Pos2>,
}

/// Pairs each of `set` with the state it had under the same id in `before`, or a fresh one.
fn kept<T, S: Default>(
    set: Vec<T>,
    before: Vec<(T, S)>,
    id_of: impl Fn(&T) -> u32,
    fresh: impl Fn(u32) -> S,
) -> Vec<(T, S)> {
    let mut before = before;
    set.into_iter()
        .map(|item| {
            let id = id_of(&item);
            let state = before
                .iter()
                .position(|(held, _)| id_of(held) == id)
                .map(|at| before.swap_remove(at).1)
                .unwrap_or_else(|| fresh(id));
            (item, state)
        })
        .collect()
}

impl Input {
    pub(crate) fn set(&mut self, walls: Vec<Wall>, regions: Vec<Region>) {
        self.walls = kept(
            walls,
            std::mem::take(&mut self.walls),
            |wall| wall.id,
            |id| WallState {
                id,
                ..WallState::default()
            },
        );
        self.regions = kept(
            regions,
            std::mem::take(&mut self.regions),
            |region| region.id,
            |id| RegionState {
                id,
                ..RegionState::default()
            },
        );
        let regions = &self.regions;
        self.touches.retain(|(_, id)| {
            regions
                .iter()
                .any(|(region, _)| region.id == *id && region.captures_touch)
        });
        if let Some(pointer) = self.pointer {
            self.placed(pointer);
        }
    }

    pub(crate) fn state(&self) -> ScreenInputState {
        ScreenInputState {
            walls: self.walls.iter().map(|(_, state)| state.clone()).collect(),
            regions: self
                .regions
                .iter()
                .map(|(_, state)| state.clone())
                .collect(),
            touch_downs: self.touch_downs,
        }
    }

    /// Moves the pointer by `delta` among `outputs`, stopped by every wall it would cross.
    pub(crate) fn moved(&mut self, position: Pos2, delta: Vec2, outputs: &[Rect]) -> Pos2 {
        let mut target = position + delta;
        for (wall, state) in &mut self.walls {
            if !on_wall(wall, position) {
                continue;
            }
            let resting = against(wall, position);
            let limit = limit(wall);
            let excess = match wall.side {
                Side::Top => limit - target.y,
                Side::Bottom => target.y - limit,
                Side::Left => limit - target.x,
                Side::Right => target.x - limit,
            };
            if excess <= 0.0 {
                continue;
            }
            match wall.side {
                Side::Top | Side::Bottom => target.y = limit,
                Side::Left | Side::Right => target.x = limit,
            }
            if resting {
                state.pushed += excess;
            }
        }
        let placed = moved(position, target - position, outputs);
        self.placed(placed);
        placed
    }

    /// The pointer is at `pointer`, however it got there.
    pub(crate) fn placed(&mut self, pointer: Pos2) {
        self.pointer = Some(pointer);
        for (wall, state) in &mut self.walls {
            state.against = against(wall, pointer);
        }
        for (region, state) in &mut self.regions {
            state.pointer = region.rect.contains(pointer);
        }
    }

    /// A finger landed at `at`: true when a region captures it, which then has it until it lifts.
    pub(crate) fn touch_down(&mut self, finger: i32, at: Pos2) -> bool {
        self.touches.retain(|(held, _)| *held != finger);
        self.touch_downs += 1;
        for (region, state) in &mut self.regions {
            if region.rect.contains(at) {
                state.touch_downs += 1;
            }
        }
        let Some((region, state)) = self
            .regions
            .iter_mut()
            .find(|(region, _)| region.captures_touch && region.rect.contains(at))
        else {
            return false;
        };
        state.touch = Some(at);
        self.touches.push((finger, region.id));
        true
    }

    /// A finger moved: true when a region has it.
    pub(crate) fn touch_moved(&mut self, finger: i32, at: Pos2) -> bool {
        let Some(state) = self.capturing(finger) else {
            return false;
        };
        state.touch = Some(at);
        true
    }

    /// A finger lifted, or its touch was cancelled: true when a region had it.
    pub(crate) fn touch_up(&mut self, finger: i32, cancelled: bool) -> bool {
        let Some(state) = self.capturing(finger) else {
            return false;
        };
        let at = state.touch.take();
        state.lifts += 1;
        state.lifted = at.filter(|_| !cancelled);
        self.touches.retain(|(held, _)| *held != finger);
        true
    }

    fn capturing(&mut self, finger: i32) -> Option<&mut RegionState> {
        let id = self
            .touches
            .iter()
            .find(|(held, _)| *held == finger)
            .map(|(_, id)| *id)?;
        self.regions
            .iter_mut()
            .find(|(region, _)| region.id == id)
            .map(|(_, state)| state)
    }
}

/// The last row or column of the display along the wall, where the pointer stops.
fn limit(wall: &Wall) -> f32 {
    let display = wall.display;
    match wall.side {
        Side::Top => display.top(),
        Side::Bottom => (display.bottom() - 1.0).max(display.top()),
        Side::Left => display.left(),
        Side::Right => (display.right() - 1.0).max(display.left()),
    }
}

fn on_wall(wall: &Wall, at: Pos2) -> bool {
    let along = match wall.side {
        Side::Top | Side::Bottom => at.x,
        Side::Left | Side::Right => at.y,
    };
    inside(wall.display, at) && along >= wall.start && along < wall.end
}

fn against(wall: &Wall, at: Pos2) -> bool {
    let across = match wall.side {
        Side::Top | Side::Bottom => at.y,
        Side::Left | Side::Right => at.x,
    };
    on_wall(wall, at) && (across - limit(wall)).abs() < 0.01
}

#[derive(Default)]
struct Shared {
    input: RefCell<Input>,
    watchers: RefCell<Vec<Rc<dyn Fn()>>>,
}

/// The screen input, handed to the program through `Setup`.
#[derive(Clone, Default)]
pub struct ScreenInput(Rc<Shared>);

impl ScreenInput {
    /// Replaces every wall and region. Each keeps what it heard while its `id` stays.
    pub fn set(&self, walls: Vec<Wall>, regions: Vec<Region>) {
        self.change(|input| input.set(walls, regions));
    }

    pub fn state(&self) -> ScreenInputState {
        self.0.input.borrow().state()
    }

    /// Hears every change to `state`.
    pub fn watch(&self, watcher: impl Fn() + 'static) {
        self.0.watchers.borrow_mut().push(Rc::new(watcher));
    }

    pub(crate) fn change<R>(&self, change: impl FnOnce(&mut Input) -> R) -> R {
        let (result, changed) = {
            let mut input = self.0.input.borrow_mut();
            let before = input.state();
            let result = change(&mut input);
            (result, input.state() != before)
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
