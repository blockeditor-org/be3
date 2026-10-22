pub mod app;
mod timeline;

block_editor_beui::beui_plugin!("../manifest.json", {
    video => app::VideoApp,
});

#[cfg(test)]
mod tests;
