pub mod app;

block_editor_beui::beui_plugin!(app::SettingsApp, "../manifest.json");

#[cfg(test)]
mod tests;
