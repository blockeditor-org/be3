use std::any::Any;
use std::sync::Arc;

use crate::color::Color32;
use crate::context::Context;
use crate::geometry::{Rect, Vec2, pos2};
use crate::input::{Event, Key, Modifiers, TouchPhase};

pub mod accessibility_dump;

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

pub trait App {
    fn update(&mut self, context: &Context, rect: Rect);

    fn clear_color(&self) -> Color32 {
        Color32::BLACK
    }

    fn setup(&mut self, _setup: &Setup) {}

    fn close_requested(&mut self) -> bool {
        true
    }

    fn exiting(&mut self) {}
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
    let typed = pending
        .iter()
        .position(|event| matches!(event, Event::Text(_) | Event::Key { .. } | Event::Ime(_)));
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

pub fn typed(old: &str, new: &str, events: &mut Vec<Event>) {
    let common = old
        .char_indices()
        .zip(new.chars())
        .find(|((_, before), after)| before != after)
        .map_or(old.len().min(new.len()), |((index, _), _)| index);
    for _ in old[common..].chars() {
        press(Key::Backspace, events);
    }
    let mut rest = &new[common..];
    while !rest.is_empty() {
        let end = rest.find(['\n', '\t']).unwrap_or(rest.len());
        if end > 0 {
            events.push(Event::Text(rest[..end].to_owned()));
        }
        match rest[end..].chars().next() {
            Some('\n') => press(Key::Enter, events),
            Some('\t') => press(Key::Tab, events),
            _ => {}
        }
        rest = rest.get(end + 1..).unwrap_or("");
    }
}

pub fn press(key: Key, events: &mut Vec<Event>) {
    for pressed in [true, false] {
        events.push(Event::Key {
            key,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
    }
}

#[cfg(test)]
mod tests;
