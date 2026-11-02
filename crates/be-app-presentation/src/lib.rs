pub mod root;

be_sdk_beui::beui_plugin!("../manifest.json", {
    presentation => root::PresentationRoot,
});

#[cfg(test)]
mod tests;
