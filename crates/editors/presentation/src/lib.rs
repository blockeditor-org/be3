pub mod app;

block_editor_plugin::beui_plugin!(app::PresentationApp, "../manifest.json");

#[cfg(test)]
mod tests;
