pub mod app;

block_editor_beui::beui_plugin!(app::DatabaseSchemaApp, "../manifest.json");

#[cfg(test)]
mod tests;
