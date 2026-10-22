pub mod app;

block_editor_beui::beui_plugin!("../manifest.json", {
    presentation => app::PresentationApp,
});

#[cfg(test)]
mod tests;
