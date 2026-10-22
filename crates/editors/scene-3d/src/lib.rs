pub mod app;
mod camera;
mod renderer;

block_editor_beui::beui_plugin!("../manifest.json", {
    scene => app::Scene3DApp,
});

#[cfg(test)]
mod tests;
