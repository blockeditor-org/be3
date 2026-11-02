pub mod app;

be_sdk_beui::beui_plugin!("../manifest.json", {
    pan_zoom => app::PanZoomApp,
});

#[cfg(test)]
mod tests;
