pub mod display_settings;
pub mod input_remaps;
pub mod input_settings;

block_editor_beui::beui_plugin!("../manifest.json", {
    block_editor_beui::be_block::InputSettingsContent => input_settings::InputSettingsApp,
    block_editor_beui::be_block::InputRemapperContent => input_remaps::InputRemapsApp,
    block_editor_beui::be_block::DisplaySettingsContent => display_settings::DisplaySettingsApp,
});
