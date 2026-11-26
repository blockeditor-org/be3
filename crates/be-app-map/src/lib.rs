mod geo;
mod points;
pub mod root;
mod tiles;

#[cfg(test)]
mod tests;

be_sdk_beui::beui_app!("../manifest.json", {
    map => root::MapRoot,
});
