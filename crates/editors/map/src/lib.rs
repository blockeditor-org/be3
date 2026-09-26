pub mod app;
mod geo;
mod mvt;
mod points;
mod raster;
mod tiles;

#[cfg(test)]
mod tests;

block_editor_beui::beui_plugin!(app::MapApp, "../manifest.json");
