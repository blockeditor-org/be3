pub mod app;

block_editor_beui::beui_plugin!(app::WorkspaceIndexApp, "../manifest.json");

#[cfg(test)]
mod tests;
