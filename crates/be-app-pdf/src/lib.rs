pub mod root;
mod pane;
mod render;

#[cfg(test)]
mod tests;

be_sdk_beui::beui_plugin!("../manifest.json", {
    pdf => root::PdfRoot,
});
