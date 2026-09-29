pub mod app;

block_editor_beui::beui_plugin!(app::BrowserTabApp, "../manifest.json");

#[cfg(test)]
mod tests;
