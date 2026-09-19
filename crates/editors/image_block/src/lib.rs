pub mod app;
pub mod decode;

block_editor_plugin::beui_plugin!(app::ImageApp, "../manifest.json");

#[cfg(test)]
mod tests;
