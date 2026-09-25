pub mod app;

block_editor_plugin::beui_plugin!(app::RepositoryApp, "../manifest.json");

#[cfg(test)]
mod tests;
