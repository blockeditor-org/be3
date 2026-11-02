pub mod root;

be_sdk_beui::beui_app!("../manifest.json", {
    pan_zoom => root::PanZoomRoot,
});

#[cfg(test)]
mod tests;
