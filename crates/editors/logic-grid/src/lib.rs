pub mod app;
pub mod compiled_logic;
mod frame;
pub mod hotbar;
pub mod logic_game;
mod renderer;

block_editor_beui::beui_plugin!("../manifest.json", {
    grid => app::LogicGridApp,
    game => logic_game::app::LogicGameApp,
    hotbar => hotbar::app::HotbarApp,
    compiled => compiled_logic::app::CompiledLogicApp,
});

#[cfg(test)]
mod tests;
