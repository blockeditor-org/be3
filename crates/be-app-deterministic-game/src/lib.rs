pub mod root;
pub mod game_module;

be_sdk_beui::beui_plugin!("../manifest.json", {
    game => root::DeterministicGameRoot,
    module => game_module::root::GameModuleRoot,
});

#[cfg(test)]
mod tests;
