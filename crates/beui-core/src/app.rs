use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

use crate::context::Context;
use crate::geometry::{Rect, Vec2, pos2};
use crate::input::{Event, TouchPhase};

pub mod accessibility_dump;
pub mod ime_mirror;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct SafeArea {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

pub fn safe_rect(screen: Vec2, area: SafeArea, scale: f32) -> Rect {
    let min = pos2(area.left / scale, area.top / scale);
    let max = pos2(
        screen.x - area.right / scale,
        screen.y - area.bottom / scale,
    );
    Rect::from_min_max(min, pos2(max.x.max(min.x), max.y.max(min.y)))
}

pub trait App: std::any::Any {
    fn update(&mut self, context: &Context, rect: Rect);

    fn setup(&mut self, _setup: &Setup) {}

    fn renderer_replaced(&mut self, _setup: &Setup) {}

    fn close_requested(&mut self) -> bool {
        true
    }

    fn exiting(&mut self) {}

    fn busy(&self) -> bool {
        false
    }

    fn environment(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    fn drawing(&self) -> bool {
        false
    }

    fn request_painting(&mut self) {}

    fn other_frames(&mut self) -> Vec<(String, FrameTimes)> {
        Vec::new()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameTimes {
    pub count: u64,
    pub longest: Duration,
}

impl FrameTimes {
    pub fn add(&mut self, took: Duration) {
        self.count += 1;
        self.longest = self.longest.max(took);
    }

    pub fn merge(&mut self, other: Self) {
        self.count += other.count;
        self.longest = self.longest.max(other.longest);
    }
}

pub struct Setup {
    pub waker: Waker,
    resources: Vec<Box<dyn Any>>,
}

impl Setup {
    pub fn new(waker: Waker) -> Self {
        Self {
            waker,
            resources: Vec::new(),
        }
    }

    pub fn provide(&mut self, resource: impl Any) {
        self.resources.push(Box::new(resource));
    }

    pub fn get<T: Any>(&self) -> Option<&T> {
        self.resources
            .iter()
            .find_map(|resource| resource.downcast_ref::<T>())
    }
}

#[derive(Clone)]
pub struct Waker(Arc<dyn Fn() + Send + Sync>);

impl Waker {
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self(Arc::new(wake))
    }

    pub fn wake(&self) {
        (self.0)();
    }
}

impl std::fmt::Debug for Waker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Waker")
    }
}

pub fn next_batch(pending: &mut Vec<Event>) -> Vec<Event> {
    let typed = pending.iter().position(|event| match event {
        Event::Key { press, .. } => !press.key.is_modifier(),
        Event::Text(_) | Event::Paste(_) | Event::Ime(_) => true,
        _ => false,
    });
    let press = typed.and_then(|start| {
        pending[start..]
            .iter()
            .position(|event| {
                matches!(
                    event,
                    Event::PointerButton { pressed: true, .. }
                        | Event::Touch {
                            phase: TouchPhase::Start,
                            ..
                        }
                        | Event::Pen {
                            phase: crate::input::PenPhase::Down(_),
                            ..
                        }
                )
            })
            .map(|offset| start + offset)
    });
    match press {
        Some(at) => {
            let rest = pending.split_off(at);
            std::mem::replace(pending, rest)
        }
        None => std::mem::take(pending),
    }
}

#[cfg(test)]
mod tests;
