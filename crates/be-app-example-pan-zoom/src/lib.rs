pub mod root;

be_sdk_beui::beui_plugin!("../manifest.json", {
    pan_zoom => root::PanZoomRoot,
});

#[cfg(test)]
mod tests;
