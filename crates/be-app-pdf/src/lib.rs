mod pane;
mod render;
pub mod root;

#[cfg(test)]
mod tests;

be_sdk_beui::beui_plugin!("../manifest.json", {
    pdf => root::PdfRoot,
});
