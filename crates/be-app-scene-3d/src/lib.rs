mod camera;
mod renderer;
pub mod root;

be_sdk_beui::beui_app!("../manifest.json", {
    scene => root::Scene3DRoot,
});

#[cfg(test)]
mod tests;
