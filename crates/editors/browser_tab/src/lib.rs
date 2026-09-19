pub mod app;

block_editor_plugin::beui_plugin!(app::BrowserTabApp, "../manifest.json");

#[cfg(test)]
mod tests;
