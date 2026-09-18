pub mod app;

block_editor_plugin::beui_plugin!(app::CompiledLogicApp, "../manifest.json");

#[cfg(test)]
mod tests;
