pub mod app;

block_editor_plugin::beui_plugin!(app::UiSettingsApp, "../manifest.json");

#[cfg(test)]
mod tests;
