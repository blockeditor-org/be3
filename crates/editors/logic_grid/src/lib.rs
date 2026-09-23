pub mod app;
mod frame;
mod renderer;

block_editor_plugin::beui_plugin!(app::LogicGridApp, "../manifest.json");

#[cfg(test)]
mod tests;
