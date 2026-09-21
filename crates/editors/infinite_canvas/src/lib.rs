pub mod app;
mod geometry;
mod images;

block_editor_plugin::beui_plugin!(app::CanvasApp, "../manifest.json");

#[cfg(test)]
mod tests;
