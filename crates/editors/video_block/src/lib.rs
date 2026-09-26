pub mod app;
mod timeline;

block_editor_beui::beui_plugin!(app::VideoApp, "../manifest.json");

#[cfg(test)]
mod tests;
