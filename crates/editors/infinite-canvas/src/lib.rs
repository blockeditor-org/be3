pub mod app;
mod freehand;
mod geometry;
mod images;
mod presence;
mod rough;

block_editor_beui::beui_plugin!(app::CanvasApp, "../manifest.json");

#[cfg(test)]
mod tests;
