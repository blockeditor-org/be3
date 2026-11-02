pub mod root;
mod freehand;
mod geometry;
mod images;
mod presence;
mod raster;
mod rough;

be_sdk_beui::beui_plugin!("../manifest.json", {
    canvas => root::CanvasRoot,
});

#[cfg(test)]
mod tests;
