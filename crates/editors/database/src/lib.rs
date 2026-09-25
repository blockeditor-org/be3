pub mod app;

block_editor_beui::beui_plugin!(app::DatabaseApp, "../manifest.json");

#[cfg(test)]
mod tests;
