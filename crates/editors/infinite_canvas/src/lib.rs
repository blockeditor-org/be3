pub mod app;
mod geometry;
mod images;
mod presence;

block_editor_beui::beui_plugin!(app::CanvasApp, "../manifest.json");

#[cfg(test)]
mod tests;
