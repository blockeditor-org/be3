pub mod app;
pub mod default_editors;
pub mod display;
pub mod input;
pub mod input_remaps;

block_editor_beui::beui_plugin!("../manifest.json", {
    settings => app::SettingsApp,
    profile => app::SettingsProfileApp,
    display => display::DisplaySettingsApp,
    input => input::InputSettingsApp,
    remaps => input_remaps::InputRemapsApp,
    defaults => default_editors::DefaultEditorsApp,
});

#[cfg(test)]
mod tests;
