pub mod app;
mod pane;
mod render;

block_editor_beui::beui_plugin!(app::PdfApp, "../manifest.json");
