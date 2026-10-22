pub mod app;

block_editor_beui::beui_plugin!("../manifest.json", {
    pan_zoom => app::PanZoomApp,
});

#[cfg(test)]
mod tests;
