pub mod app;
pub mod document;
mod hex;
pub mod presence;

block_editor_beui::beui_plugin!("../manifest.json", {
    text => app::TextApp,
});
