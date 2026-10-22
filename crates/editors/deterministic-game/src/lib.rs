pub mod app;
pub mod game_module;

block_editor_beui::beui_plugin!("../manifest.json", {
    game => app::DeterministicGameApp,
    module => game_module::app::GameModuleApp,
});

#[cfg(test)]
mod tests;
