extern crate self as beui;

pub mod actions;
pub mod animation;
pub mod colors;
pub mod component_ref;
pub mod components;
pub mod drag;
pub mod file_picker;
pub mod file_saver;
#[cfg(beui_html)]
mod html;
pub mod measure;
#[cfg(not(beui_html))]
mod native;
pub mod nodes;
mod painter;
pub mod primitives;
pub mod reactive;
mod text_children;
pub mod window;

#[cfg(beui_html)]
use html as core;
#[cfg(not(beui_html))]
use native as core;

pub use beui_tree::{child_type, value_child_type};
pub use beui_types as types;

#[cfg(beui_html)]
pub mod running;

#[cfg(not(beui_html))]
pub mod native_api;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Core {
    Native,
    Html,
}

pub const CORE: Core = if cfg!(beui_html) {
    Core::Html
} else {
    Core::Native
};
