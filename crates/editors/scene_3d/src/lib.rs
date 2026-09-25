pub mod app;
mod camera;
mod renderer;

block_editor_plugin::plugin!(app::Scene3D, "../manifest.json");

#[cfg(test)]
mod tests;
