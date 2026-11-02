pub mod root;
pub mod default_editors;
pub mod display;
pub mod home;
pub mod input;
pub mod input_remaps;
pub mod prefixes;
pub mod tags;

be_sdk_beui::beui_plugin!("../manifest.json", {
    settings => root::SettingsRoot,
    profile => root::SettingsProfileRoot,
    display => display::DisplaySettingsRoot,
    input => input::InputSettingsRoot,
    remaps => input_remaps::InputRemapsRoot,
    defaults => default_editors::DefaultEditorsRoot,
    tags => tags::TagsRoot,
    prefixes => prefixes::PrefixesRoot,
    home => home::HomeRoot,
});

#[cfg(test)]
mod tests;
