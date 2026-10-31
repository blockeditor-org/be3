pub mod app;
mod freehand;
mod geometry;
mod images;
mod presence;
mod raster;
mod rough;

block_editor_beui::beui_plugin!("../manifest.json", {
    canvas => app::CanvasApp,
});

#[cfg(test)]
mod tests;
