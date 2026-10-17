pub mod app;
pub mod ui_settings;

block_editor_beui::beui_plugin!("../manifest.json", {
    block_editor_beui::be_block::SettingsContent => app::SettingsApp,
    block_editor_beui::be_block::SettingsProfileContent => app::SettingsProfileApp,
    block_editor_beui::be_block::UiSettingsContent => ui_settings::UiSettingsApp,
    block_editor_beui::be_block::EditorSettingsContent => ui_settings::DefaultEditorsApp,
});

#[cfg(test)]
mod tests;
