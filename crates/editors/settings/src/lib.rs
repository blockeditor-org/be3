pub mod app;

block_editor_beui::beui_plugin!("../manifest.json", {
    block_editor_beui::be_block::SettingsContent => app::SettingsApp,
    block_editor_beui::be_block::SettingsProfileContent => app::SettingsProfileApp,
});

#[cfg(test)]
mod tests;
