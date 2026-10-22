pub mod app;
mod geometry;
mod overlay;
mod raytracer;

#[cfg(test)]
mod tests;

block_editor_beui::beui_plugin!("../manifest.json", {
    ray_tracer => app::PixelRayTracerApp,
});
