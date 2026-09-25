use std::sync::Arc;

use crate::color::Color32;
use crate::context::Context;
use crate::geometry::{Rect, Vec2};
use crate::input::{Event, Key, Modifiers, TouchPhase};

#[cfg(feature = "window")]
mod clipboard;
#[cfg(feature = "window")]
mod native;
#[cfg(all(feature = "window", target_os = "android"))]
mod soft_keyboard;
#[cfg(feature = "web")]
mod web;

#[cfg(feature = "window")]
pub use native::{run, run_with, set_safe_area};
#[cfg(feature = "web")]
pub use web::run_web;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct SafeArea {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
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
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub format: wgpu::TextureFormat,
    pub waker: Waker,
    #[cfg(feature = "window")]
    pub window: Arc<winit::window::Window>,
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

#[cfg(feature = "render")]
pub type OpenDevice = Arc<
    dyn Fn(&wgpu::Adapter, &wgpu::DeviceDescriptor<'_>) -> Option<(wgpu::Device, wgpu::Queue)>
        + Send
        + Sync,
>;

pub struct RunOptions {
    pub title: String,
    pub app_id: Option<String>,
    pub size: Vec2,
    #[cfg(feature = "render")]
    pub open_device: Option<OpenDevice>,
    #[cfg(all(feature = "window", target_os = "android"))]
    pub android_app: Option<winit::platform::android::activity::AndroidApp>,
}

impl RunOptions {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            app_id: None,
            size: Vec2::new(1280.0, 800.0),
            #[cfg(feature = "render")]
            open_device: None,
            #[cfg(all(feature = "window", target_os = "android"))]
            android_app: None,
        }
    }
}

#[cfg_attr(not(any(feature = "window", feature = "web")), allow(dead_code))]
pub(crate) fn next_batch(pending: &mut Vec<Event>) -> Vec<Event> {
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

#[cfg_attr(not(all(feature = "window", target_os = "android")), allow(dead_code))]
pub(crate) fn typed(old: &str, new: &str, events: &mut Vec<Event>) {
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

fn press(key: Key, events: &mut Vec<Event>) {
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
