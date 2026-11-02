pub mod app;
mod geo;
mod mvt;
mod points;
mod raster;
mod tiles;

#[cfg(test)]
mod tests;

be_sdk_beui::beui_plugin!("../manifest.json", {
    map => app::MapApp,
});
