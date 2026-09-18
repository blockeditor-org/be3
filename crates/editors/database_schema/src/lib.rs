pub mod app;

block_editor_plugin::beui_plugin!(app::DatabaseSchemaApp, "../manifest.json");

#[cfg(test)]
mod tests;
