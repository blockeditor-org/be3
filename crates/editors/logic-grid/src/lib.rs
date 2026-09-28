pub mod app;
pub mod compiled_logic;
mod frame;
pub mod hotbar;
pub mod logic_game;
mod renderer;

block_editor_beui::beui_plugin!("../manifest.json", {
    block_editor_beui::be_block::LogicGridContent => app::LogicGridApp,
    block_editor_beui::be_block::LogicGameContent => logic_game::app::LogicGameApp,
    block_editor_beui::be_block::HotbarContent => hotbar::app::HotbarApp,
    block_editor_beui::be_block::CompiledLogicContent => compiled_logic::app::CompiledLogicApp,
});

#[cfg(test)]
mod tests;
