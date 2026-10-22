pub mod app;
pub mod decode;

block_editor_beui::beui_plugin!("../manifest.json", {
    image => app::ImageApp,
});

#[cfg(test)]
mod tests;
