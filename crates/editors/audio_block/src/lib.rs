pub mod app;

block_editor_beui::beui_plugin!(app::AudioApp, "../manifest.json");

#[cfg(test)]
mod tests;
