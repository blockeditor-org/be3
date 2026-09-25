pub mod app;
mod binary_addition;

block_editor_beui::beui_plugin!(app::LogicGameApp, "../manifest.json");

#[cfg(test)]
mod tests;
