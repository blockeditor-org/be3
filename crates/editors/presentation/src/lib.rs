pub mod app;

block_editor_beui::beui_plugin!(app::PresentationApp, "../manifest.json");

#[cfg(test)]
mod tests;
