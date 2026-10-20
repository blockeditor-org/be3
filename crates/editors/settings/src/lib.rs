pub mod app;
pub mod default_editors;
pub mod display;

block_editor_beui::beui_plugin!("../manifest.json", {
    block_editor_beui::be_block::SettingsContent => app::SettingsApp,
    block_editor_beui::be_block::SettingsProfileContent => app::SettingsProfileApp,
    block_editor_beui::be_block::DisplaySettingsContent => display::DisplaySettingsApp,
    block_editor_beui::be_block::EditorSettingsContent => default_editors::DefaultEditorsApp,
});

#[cfg(test)]
mod tests;
