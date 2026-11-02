pub mod app;

#[cfg(test)]
mod tests;

be_sdk_beui::beui_plugin!("../manifest.json", {
    files => app::FileTreeApp,
});
