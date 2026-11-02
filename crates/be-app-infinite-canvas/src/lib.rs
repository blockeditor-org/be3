pub mod app;
mod freehand;
mod geometry;
mod images;
mod presence;
mod raster;
mod rough;

be_sdk_beui::beui_plugin!("../manifest.json", {
    canvas => app::CanvasApp,
});

#[cfg(test)]
mod tests;
