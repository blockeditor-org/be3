pub mod app;

block_editor_beui::beui_plugin!(app::BeuiGalleryApp, "../manifest.json");

#[cfg(test)]
mod tests;
