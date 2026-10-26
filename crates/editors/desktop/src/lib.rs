pub mod app;

block_editor_beui::beui_plugin!("../manifest.json", {
    desktop => app::DesktopApp,
});

#[cfg(test)]
mod tests;
