pub use beui::*;

pub mod native;

#[cfg(all(feature = "web", target_arch = "wasm32"))]
mod web;

mod keyboard_host;

#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
mod runner;
#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
mod saved_preferences;

#[cfg(test)]
#[allow(hidden_glob_reexports)]
use beui_core::{
    base, color, context, damage, draw, drawing, filter, flash, font, geometry, image, input,
    interact, node, painter, screen_simulation, sight,
};
#[cfg(test)]
use beui_inspector::{self as inspector, mouse_simulation};

#[cfg(test)]
mod tests;
