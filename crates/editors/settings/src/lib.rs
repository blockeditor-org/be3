pub mod app;
pub mod default_editors;
pub mod display;
pub mod home;
pub mod input;
pub mod input_remaps;
pub mod prefixes;
pub mod tags;

block_editor_beui::beui_plugin!("../manifest.json", {
    settings => app::SettingsApp,
    profile => app::SettingsProfileApp,
    display => display::DisplaySettingsApp,
    input => input::InputSettingsApp,
    remaps => input_remaps::InputRemapsApp,
    defaults => default_editors::DefaultEditorsApp,
    tags => tags::TagsApp,
    prefixes => prefixes::PrefixesApp,
    home => home::HomeApp,
});

#[cfg(test)]
mod tests;
