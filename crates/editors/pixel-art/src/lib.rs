pub mod app;
pub mod artifact;
pub mod canvas;
pub mod color;
pub mod drawing;
pub mod panels;

block_editor_beui::beui_plugin!("../manifest.json", {
    pixel_art => app::PixelArtApp,
});

#[cfg(test)]
mod tests;
