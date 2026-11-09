mod api;
pub mod root;
mod time;

be_sdk_beui::beui_app!("../manifest.json", {
    boat => root::BoatRoot,
});

#[cfg(test)]
mod tests;
