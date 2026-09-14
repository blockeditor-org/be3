pub mod app;

block_editor_plugin::plugin!(app::WorkspaceUiApp, "../manifest.json");

#[cfg(test)]
mod tests;
