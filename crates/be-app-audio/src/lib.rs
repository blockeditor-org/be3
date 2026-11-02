pub mod root;

be_sdk_beui::beui_app!("../manifest.json", {
    audio => root::AudioRoot,
});

#[cfg(test)]
mod tests;
