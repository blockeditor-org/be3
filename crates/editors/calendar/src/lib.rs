pub mod app;

block_editor_beui::beui_plugin!(app::CalendarApp, "../manifest.json");

#[cfg(test)]
mod tests;
