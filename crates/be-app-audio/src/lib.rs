pub mod app;

be_sdk_beui::beui_plugin!("../manifest.json", {
    audio => app::AudioApp,
});

#[cfg(test)]
mod tests;
