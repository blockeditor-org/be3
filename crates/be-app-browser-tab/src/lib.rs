pub mod root;

be_sdk_beui::beui_plugin!("../manifest.json", {
    browser => root::BrowserTabRoot,
});

#[cfg(test)]
mod tests;
