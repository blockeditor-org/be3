pub mod app;
mod geometry;
mod images;
mod presence;

block_editor_plugin::beui_plugin!(app::CanvasApp, "../manifest.json");

#[cfg(test)]
mod tests;
