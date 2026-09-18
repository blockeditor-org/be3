pub mod app;

block_editor_plugin::beui_plugin!(app::GameModuleApp, "../manifest.json");

#[cfg(test)]
mod tests;
