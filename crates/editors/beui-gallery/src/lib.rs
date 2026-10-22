pub mod app;

block_editor_beui::beui_plugin!("../manifest.json", {
    gallery => app::BeuiGalleryApp,
});

#[cfg(test)]
mod tests;
