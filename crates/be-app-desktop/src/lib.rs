pub mod app;

be_sdk_beui::beui_plugin!("../manifest.json", {
    desktop => app::DesktopApp,
});

#[cfg(test)]
mod tests;
