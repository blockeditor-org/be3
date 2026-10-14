pub mod app;

block_editor_beui::beui_plugin!("../manifest.json", {
    block_editor_beui::be_block::UiSettingsContent => app::UiSettingsApp,
    block_editor_beui::be_block::EditorSettingsContent => app::DefaultEditorsApp,
});

#[cfg(test)]
mod tests;
