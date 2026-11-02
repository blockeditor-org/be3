pub mod app;
pub mod decode;

be_sdk_beui::beui_plugin!("../manifest.json", {
    image => app::ImageApp,
});

#[cfg(test)]
mod tests;
