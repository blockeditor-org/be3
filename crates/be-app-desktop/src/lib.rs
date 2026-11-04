pub mod root;

be_sdk_beui::beui_app!("../manifest.json", {
    desktop => root::DesktopRoot,
});

#[cfg(test)]
mod tests;
