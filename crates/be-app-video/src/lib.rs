pub mod root;
mod timeline;

be_sdk_beui::beui_app!("../manifest.json", {
    video => root::VideoRoot,
});

#[cfg(test)]
mod tests;
