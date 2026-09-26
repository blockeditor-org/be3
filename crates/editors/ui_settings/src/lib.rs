pub mod app;

block_editor_beui::beui_plugin!(app::UiSettingsApp, "../manifest.json");

#[cfg(test)]
mod tests;
