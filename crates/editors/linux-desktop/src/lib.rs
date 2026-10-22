pub mod app;

block_editor_beui::beui_plugin!("../manifest.json", {
    desktop => app::LinuxDesktopApp,
});

#[cfg(test)]
mod tests;
