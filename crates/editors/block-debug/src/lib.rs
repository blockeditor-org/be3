pub mod app;

block_editor_beui::beui_plugin!(app::BlockDebugApp, "../manifest.json");

#[cfg(test)]
mod tests;
