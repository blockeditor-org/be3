pub mod app;
pub mod document;
mod hex;
pub mod presence;

be_sdk_beui::beui_plugin!("../manifest.json", {
    text => app::TextApp,
});
