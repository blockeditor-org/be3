pub mod compiled_logic;
mod frame;
pub mod hotbar;
pub mod logic_game;
mod renderer;
pub mod root;

be_sdk_beui::beui_plugin!("../manifest.json", {
    grid => root::LogicGridRoot,
    game => logic_game::root::LogicGameRoot,
    hotbar => hotbar::root::HotbarRoot,
    compiled => compiled_logic::root::CompiledLogicRoot,
});

#[cfg(test)]
mod tests;
