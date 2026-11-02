pub mod app;

be_sdk_beui::beui_plugin!("../manifest.json", {
    presentation => app::PresentationApp,
});

#[cfg(test)]
mod tests;
