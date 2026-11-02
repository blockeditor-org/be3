pub mod decode;
pub mod root;

be_sdk_beui::beui_plugin!("../manifest.json", {
    image => root::ImageRoot,
});

#[cfg(test)]
mod tests;
