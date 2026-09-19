pub mod app;

block_editor_plugin::beui_plugin!(app::WorkspaceIndexApp, "../manifest.json");

#[cfg(test)]
mod tests;
