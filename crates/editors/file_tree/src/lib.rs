pub mod app;

#[cfg(test)]
mod tests;

block_editor_plugin::beui_plugin!(app::FileTreeApp, "../manifest.json");
