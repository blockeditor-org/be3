pub mod app;

block_editor_beui::beui_plugin!(app::LinuxDesktopApp, "../manifest.json");

#[cfg(test)]
mod tests;
