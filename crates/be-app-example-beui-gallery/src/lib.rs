pub mod root;

be_sdk_beui::beui_plugin!("../manifest.json", {
    gallery => root::BeuiGalleryRoot,
});

#[cfg(test)]
mod tests;
