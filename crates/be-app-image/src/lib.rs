pub mod root;
pub mod decode;

be_sdk_beui::beui_plugin!("../manifest.json", {
    image => root::ImageRoot,
});

#[cfg(test)]
mod tests;
