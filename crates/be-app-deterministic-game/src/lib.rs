pub mod app;
pub mod game_module;

be_sdk_beui::beui_plugin!("../manifest.json", {
    game => app::DeterministicGameApp,
    module => game_module::app::GameModuleApp,
});

#[cfg(test)]
mod tests;
