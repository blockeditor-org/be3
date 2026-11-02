pub mod root;

be_sdk_beui::beui_app!("../manifest.json", {
    gallery => root::BeuiGalleryRoot,
});

#[cfg(test)]
mod tests;
