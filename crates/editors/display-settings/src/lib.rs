pub mod app;

block_editor_beui::beui_plugin!(app::DisplaySettingsApp, "../manifest.json");

#[cfg(test)]
mod tests;
