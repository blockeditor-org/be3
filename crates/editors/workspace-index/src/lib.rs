pub mod app;

block_editor_beui::beui_plugin!("../manifest.json", {
    block_editor_beui::be_block::FolderContent => app::WorkspaceIndexApp,
    block_editor_beui::be_block::WorkspaceContent => app::WorkspaceIndexApp,
});

#[cfg(test)]
mod tests;
