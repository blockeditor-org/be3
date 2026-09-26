pub mod app;

block_editor_beui::beui_plugin!(app::ChecklistApp, "../manifest.json");

#[cfg(test)]
mod tests;
