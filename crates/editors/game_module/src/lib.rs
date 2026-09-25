pub mod app;

block_editor_beui::beui_plugin!(app::GameModuleApp, "../manifest.json");

#[cfg(test)]
mod tests;
