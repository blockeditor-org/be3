extern crate self as beui;

#[cfg(not(beui_html))]
pub mod actions;
#[cfg(not(beui_html))]
pub mod component_ref;
#[cfg(not(beui_html))]
pub mod components;
#[cfg(not(beui_html))]
pub mod file_picker;
#[cfg(not(beui_html))]
pub mod file_saver;
#[cfg(not(beui_html))]
mod native;
#[cfg(not(beui_html))]
pub mod nodes;
#[cfg(not(beui_html))]
pub mod primitives;
#[cfg(not(beui_html))]
pub mod reactive;
#[cfg(not(beui_html))]
pub mod selection;

#[cfg(not(beui_html))]
use native as core;

#[cfg(not(beui_html))]
pub use beui_core::node;
pub use beui_tree::{child_type, value_child_type};
pub use beui_types as types;

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
