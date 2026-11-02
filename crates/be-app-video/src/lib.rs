pub mod app;
mod timeline;

be_sdk_beui::beui_plugin!("../manifest.json", {
    video => app::VideoApp,
});

#[cfg(test)]
mod tests;
