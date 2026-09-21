pub mod app;
mod camera;
mod renderer;

block_editor_plugin::beui_plugin!(app::Scene3DApp, "../manifest.json");

#[cfg(test)]
mod tests;
