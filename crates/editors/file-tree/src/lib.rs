pub mod app;

#[cfg(test)]
mod tests;

block_editor_beui::beui_plugin!(app::FileTreeApp, "../manifest.json");
