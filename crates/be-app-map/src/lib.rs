pub mod root;
mod geo;
mod mvt;
mod points;
mod raster;
mod tiles;

#[cfg(test)]
mod tests;

be_sdk_beui::beui_plugin!("../manifest.json", {
    map => root::MapRoot,
});
