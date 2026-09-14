pub mod app;

block_editor_plugin::beui_plugin!(app::DeterministicGameApp, "../manifest.json");

#[cfg(test)]
mod tests;
