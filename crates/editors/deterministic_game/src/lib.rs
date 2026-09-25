pub mod app;

block_editor_beui::beui_plugin!(app::DeterministicGameApp, "../manifest.json");

#[cfg(test)]
mod tests;
