pub mod root;

be_sdk_beui::beui_app!("../manifest.json", {
    browser => root::BrowserTabRoot,
});

#[cfg(test)]
mod tests;
