pub mod app;
mod pane;
mod render;

#[cfg(test)]
mod tests;

block_editor_beui::beui_plugin!("../manifest.json", {
    pdf => app::PdfApp,
});
