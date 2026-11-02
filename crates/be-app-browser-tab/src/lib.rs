pub mod app;

be_sdk_beui::beui_plugin!("../manifest.json", {
    browser => app::BrowserTabApp,
});

#[cfg(test)]
mod tests;
