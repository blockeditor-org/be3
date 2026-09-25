pub mod app;

block_editor_beui::beui_plugin!(app::CompiledLogicApp, "../manifest.json");

#[cfg(test)]
mod tests;
