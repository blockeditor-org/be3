pub mod document;
mod hex;
pub mod presence;
pub mod root;

be_sdk_beui::beui_app!("../manifest.json", {
    text => root::TextRoot,
});
