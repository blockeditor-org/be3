use std::any::Any;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::context::Context;
use beui_types::TouchPhase;
use beui_types::{Event, Input, WindowId};
use beui_types::{Rect, Vec2, pos2};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    Background,
    Foreground,
    Focused,
    NetworkChanged,
}

pub trait Program: std::any::Any {
    fn update(&mut self, context: &Context, rect: Rect);

    fn setup(&mut self, _setup: &Setup) {}

    fn renderer_replaced(&mut self, _setup: &Setup) {}

    fn close_requested(&mut self) -> bool {
        true
    }

    fn exiting(&mut self) {}

    /// Whether the program takes `input` for `window` itself, before beui makes the document's
    /// events of it: the platform's input after the layers' (`Layer::input`), in the window's
    /// points, each offered once, in order, with `time`, when the platform sampled it if it said
    /// (pens and touches mostly do). It is the intake's queue, so a run of moves is merged to
    /// its last within a frame unless the window asks for every sample
    /// (`Context::want_every_sample`). What it takes no document hears, though the intake still follows the
    /// pointer, its buttons and the modifiers through it. A program that routes its windows'
    /// input itself (be-host's root) takes all of it: IME composition, touches and pens as the
    /// platform reported them, and every window's.
    ///
    /// A program that takes only part of a stream leaves its document's view of it
    /// inconsistent: under touch emulation, a press the document heard whose release or
    /// `PointersCancelled` the program took leaves the emulated finger down with no end or
    /// cancel; and a taken `Modifiers` or `Focus(false)` leaves the document's modifiers as
    /// they were until it next hears them change.
    fn input(&mut self, _window: WindowId, _input: &Input, _time: Option<Instant>) -> bool {
        false
    }

    fn lifecycle(&mut self, _change: Lifecycle) {}

    fn busy(&self) -> Vec<String> {
        Vec::new()
    }

    /// What the program still waits on outside its window before it is in step with it, one
    /// reason each: `None` for a program with nothing outside to keep in step with (be-host
    /// names its server). drive's `synced` waits until it gives none.
    fn out_of_step(&self) -> Option<Vec<String>> {
        None
    }

    /// Whether the program is connected to what it keeps in step with outside its window:
    /// `None` for a program with nothing outside. drive's `offline` waits until it is not.
    fn connected(&self) -> Option<bool> {
        None
    }

    /// While the frame clock is stopped, when the soonest of the program's own timers for
    /// keeping in step comes due on it (a save after a quiet spell, a retry's backoff), which
    /// cannot happen until the clock gets there: drive's `synced` moves a stopped clock to it
    /// once the program is quiet but still out of step.
    fn in_step_due(&self) -> Option<Instant> {
        None
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

#[derive(Clone, Copy, Debug)]
pub struct NativeWindow(pub raw_window_handle::RawWindowHandle);

impl NativeWindow {
    pub fn win32(&self) -> Option<isize> {
        match self.0 {
            raw_window_handle::RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
            _ => None,
        }
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
                            phase: beui_types::PenPhase::Down(_),
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
