pub mod app;

block_editor_beui::beui_plugin!("../manifest.json", {
    workspace => app::WorkspaceUiApp,
});

#[cfg(test)]
mod tests;
