pub mod app;
pub mod document;
mod hex;
pub mod presence;

block_editor_plugin::beui_plugin!(app::TextApp, "../manifest.json");
