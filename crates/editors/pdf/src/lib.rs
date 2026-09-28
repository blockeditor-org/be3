pub mod app;
mod pane;
mod render;

#[cfg(test)]
mod tests;

block_editor_beui::beui_plugin!(app::PdfApp, "../manifest.json");
