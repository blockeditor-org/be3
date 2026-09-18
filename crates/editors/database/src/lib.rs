pub mod app;

block_editor_plugin::beui_plugin!(app::DatabaseApp, "../manifest.json");

#[cfg(test)]
mod tests;
