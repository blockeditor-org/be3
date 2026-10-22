pub mod app;

block_editor_beui::beui_plugin!("../manifest.json", {
    folder => app::WorkspaceIndexApp,
    workspace => app::WorkspaceIndexApp,
});

#[cfg(test)]
mod tests;
