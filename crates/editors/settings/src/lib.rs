pub mod app;

block_editor_plugin::beui_plugin!(app::SettingsApp, "../manifest.json");

#[cfg(test)]
mod tests;
