pub mod app;

be_sdk_beui::beui_plugin!("../manifest.json", {
    gallery => app::BeuiGalleryApp,
});

#[cfg(test)]
mod tests;
