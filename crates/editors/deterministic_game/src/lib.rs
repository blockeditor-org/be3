pub mod app;
pub mod game_module;

block_editor_beui::beui_plugin!("../manifest.json", {
    block_editor_beui::be_block::DeterministicGameContent => app::DeterministicGameApp,
    block_editor_beui::be_block::GameModuleContent => game_module::app::GameModuleApp,
});

#[cfg(test)]
mod tests;
