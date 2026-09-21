pub mod app;
pub mod document;
mod hex;
mod layout;
mod palette;
pub mod presence;

block_editor_plugin::beui_plugin!(app::TextApp, "../manifest.json");
