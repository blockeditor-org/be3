pub mod app;

block_editor_beui::beui_plugin!("../manifest.json", {
    browser => app::BrowserTabApp,
});

#[cfg(test)]
mod tests;
