pub mod app;

block_editor_beui::beui_plugin!(app::WorkspaceUiApp, "../manifest.json");

#[cfg(test)]
mod tests;
