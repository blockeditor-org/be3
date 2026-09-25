pub mod app;
pub mod download;
pub mod render;
pub mod view;

#[cfg(test)]
mod tests;

block_editor_beui::beui_plugin!(app::PaintReviewApp, "../manifest.json");
