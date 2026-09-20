pub mod app;
mod geometry;
mod overlay;
mod raytracer;

#[cfg(test)]
mod tests;

block_editor_plugin::beui_plugin!(app::PixelRayTracerApp, "../manifest.json");
