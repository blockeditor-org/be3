pub mod app;

block_editor_beui::beui_plugin!(app::PanZoomApp, "../manifest.json");

#[cfg(test)]
mod tests;
