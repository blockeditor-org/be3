pub mod app;

block_editor_beui::beui_plugin!("../manifest.json", {
    audio => app::AudioApp,
});

#[cfg(test)]
mod tests;
