pub mod app;
pub mod sort;

block_editor_beui::beui_plugin!(app::DatabaseViewApp, "../manifest.json");

#[cfg(test)]
mod tests;
