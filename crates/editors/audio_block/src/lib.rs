pub mod app;

block_editor_plugin::beui_plugin!(app::AudioApp, "../manifest.json");

#[cfg(test)]
mod tests;
