pub mod app;
mod pane;
mod render;

block_editor_plugin::beui_plugin!(app::PdfApp, "../manifest.json");
