#[cfg(target_arch = "wasm32")]
pub use beui_core_html::automate::automate;
pub use beui_core_html::page::choose_theme;
pub use beui_core_html::run::{STYLES_ID, stylesheet};

use crate::primitives::Root;

pub fn render_to_string(root: impl FnOnce() -> Root) -> String {
    beui_core_html::run::render_to_string(|| root().child())
}

#[cfg(target_arch = "wasm32")]
pub fn run(element_id: &str, root: impl FnOnce() -> Root) -> Result<(), String> {
    beui_core_html::run::run(element_id, || root().child())
}

#[cfg(target_arch = "wasm32")]
pub fn hydrate(element_id: &str, root: impl FnOnce() -> Root) -> Result<(), String> {
    beui_core_html::run::hydrate(element_id, || root().child())
}
