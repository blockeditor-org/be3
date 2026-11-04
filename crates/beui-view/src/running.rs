#[cfg(target_arch = "wasm32")]
pub use beui_core_html::automate::automate;
pub use beui_core_html::page::choose_theme;
pub use beui_core_html::run::{STYLES_ID, stylesheet};

use crate::primitives::Root;

/// Renders a page that shows before any script runs, from a program built for
/// `linux_x86_64_html` (at build time with `prerendered_page` in buck/program/defs.bzl,
/// or per request). Serve it with `stylesheet(font_base)` in a `<style id=STYLES_ID>`
/// and call `hydrate` with the same view in the browser.
pub fn render_to_string(root: impl FnOnce() -> Root) -> String {
    beui_core_html::run::render_to_string(|| root().child())
}

/// Reads the browser's preferences before building, as `hydrate` does, so the first frame
/// matches the page.
#[cfg(target_arch = "wasm32")]
pub fn run(element_id: &str, root: impl FnOnce() -> Root) -> Result<(), String> {
    beui_core_html::run::run(element_id, || root().child())
}

#[cfg(target_arch = "wasm32")]
pub fn hydrate(element_id: &str, root: impl FnOnce() -> Root) -> Result<(), String> {
    beui_core_html::run::hydrate(element_id, || root().child())
}
