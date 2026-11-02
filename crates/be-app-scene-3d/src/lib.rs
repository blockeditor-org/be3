mod camera;
mod renderer;
pub mod root;

be_sdk_beui::beui_plugin!("../manifest.json", {
    scene => root::Scene3DRoot,
});

#[cfg(test)]
mod tests;
