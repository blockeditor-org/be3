mod freehand;
mod geometry;
mod images;
mod presence;
mod raster;
pub mod root;
mod rough;

be_sdk_beui::beui_plugin!("../manifest.json", {
    canvas => root::CanvasRoot,
});

#[cfg(test)]
mod tests;
