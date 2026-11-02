pub mod game_module;
pub mod root;

be_sdk_beui::beui_app!("../manifest.json", {
    game => root::DeterministicGameRoot,
    module => game_module::root::GameModuleRoot,
});

#[cfg(test)]
mod tests;
